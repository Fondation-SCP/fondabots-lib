use errors::Error;
use serenity::all::Context as SerenityContext;
use serenity::all::Message;
use serenity::all::{ChannelId, GetMessages, MessageId, UserId};
use serenity::futures::future::{join_all, try_join_all};
use std::collections::HashMap;
use tools::PreloadedChannel;

use crate::tools::Preloaded;
use crate::Bot;
use crate::ErrType;
use crate::{errors, tools, Object};

/* Attention: le test doit gérer les erreurs seul, il est possible que l’écrit n’existe pas. */
pub struct Affichan<T: Object> {
    chan: PreloadedChannel,
    messages: HashMap<u64, Message>,
    test: Box<dyn Fn(Option<&T>) -> bool + Sync + Send + 'static>
}

impl<T: Object> Affichan<T> {
    pub fn new(chan: ChannelId, test: Box<dyn Fn(&T) -> bool + Sync + Send + 'static>) -> Self {
        Self {
            chan: PreloadedChannel::Unloaded(chan),
            messages: HashMap::new(),
            test: Box::new(move |ecrit| {
                if let Some(ecrit) = ecrit {
                    test(ecrit)
                } else {false}
            })
        }
    }

    pub async fn load(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        self.chan = PreloadedChannel::Loaded(self.chan.load(ctx).await?);
        Ok(())
    }

    pub async fn init(&mut self, database: &HashMap<u64, T>, self_id: &UserId, ctx: &SerenityContext) -> Result<(), ErrType> {
        self.load(ctx).await?;
        let mut messages = self.chan.get()?.messages(ctx, GetMessages::new().limit(100)).await?;
        while !messages.is_empty() {
            let last_message_id = messages.last().unwrap().id.get(); // The loop makes None impossible
            while !messages.is_empty() {
                let message = messages.pop().unwrap(); // The loop makes None impossible
                if message.author.id.get() != self_id.get() || message.embeds.is_empty() {
                    continue;
                }

                let embed_footer = match &message.embeds.get(0).ok_or(Error::Generic)?.footer {
                    Some(footer) => footer,
                    None => {
                        eprintln!("Embed sans footer: message {} ignoré.", message.id);
                        continue;
                    }
                };

                let footer_text = match embed_footer.text.parse() {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("{e}: message {} ignoré.", message.id);
                        continue;
                    }
                };

                if let Some(object) = database.get(&footer_text) {
                    if !self.messages.contains_key(&object.get_id()) {
                        self.messages.insert(object.get_id(), message);
                    } else {
                        eprintln!("Message {} en trop: suppression.", message.id);
                        message.delete(ctx).await?;
                    }
                } else {
                    message.delete(ctx).await?;
                    eprintln!("Message {} sans objet associé: message supprimé.", message.id);
                    continue;
                }


            }
            messages = self.chan.get()?.messages(ctx, GetMessages::new().limit(100).before(last_message_id)).await?;
        }

        self.update(database, ctx).await
    }

    pub async fn update(&mut self, database: &HashMap<u64, T>, ctx: &SerenityContext) -> Result<(), ErrType> {
        vec![
            /* Messages qui ne correspondent plus au test ou ne sont plus dans la base de données */
            join_all(self.messages.iter_mut().filter(|(object_id, _)|
                !(database.contains_key(object_id) && (self.test)(database.get(object_id)))
            ).map(|(object_id, message)| async {
                let _ = message.delete(ctx).await; *object_id
            })).await,

            /* Messages dont la modification a échoué */
            join_all(self.messages.iter_mut().filter(|(object_id, _)|
                /* Condition excluant les objes déjà traités au-dessus et s’assurant que
                   get(object_id) ne retournera pas None */
                if database.contains_key(object_id) && (self.test)(database.get(object_id)) {
                    database.get(object_id).unwrap().is_modified()
                } else {
                    false
                }
            ).map(|(object_id, message)| async {
                if message.edit(ctx, database.get(object_id).unwrap().get_message_edit()).await.is_err() {
                    Some(*object_id)
                } else {
                    None
                }
            })).await.into_iter().filter_map(|x| x).collect()
        ].concat().iter().for_each(|id| {
            self.messages.remove(id);
        });

        let self_chan = &self.chan;

        try_join_all(
            tools::sort_by_date(
                database.iter()
                .filter(| (id, obj) |
                    (self.test)(Some(obj)) && !self.messages.contains_key(id)
                ).collect()
            ).into_iter().rev().map(|(object_id, object)| async move {
                match self_chan.get() {
                    Ok(chan) => match chan.send_message(ctx, object.get_message()).await {
                        Ok(message) => Ok((object_id, message)),
                        Err(e) => Err(ErrType::LibError(Box::new(e)))
                    }
                    Err(e) => Err(e)
                }
            })
        ).await?.into_iter().for_each(|(&object_id, message)| {
            self.messages.insert(object_id, message);
        });

        Ok(())
    }

    pub async fn purge(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        self.refresh(ctx).await?;
        self.messages.clear();
        Ok(())
    }

    pub async fn refresh(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        try_join_all(self.messages.iter_mut().map(|(_, message)| message.delete(ctx))).await?;
        Ok(())
    }

    pub async fn check_message_deletion(&self, bot: &Bot<T>, ctx: &SerenityContext, message_id: &MessageId) -> Result<(), ErrType> {
        try_join_all(
            self.messages.iter().filter(|(_, message)| message.id.get() == message_id.get())
                /* Ne peut trouver qu’un seul résultat, mais on fait comme si quand-même */
                .map(|(object_id, _)| async {
                    match self.chan.get() {
                        Ok(chan) => match bot.database.get(object_id) {
                            Some(object) => chan.send_message(ctx, object.get_message()).await.or_else(|err| Err(Error::LibError(Box::new(err)))),
                            None => Err(Error::ObjectNotFound(format!("Objet {} référencé dans un message supprimé dans Affichan {} (id: {})", *object_id, chan.name, chan.id)))
                        }
                        Err(e) => Err(e)
                    }
                })
        ).await?;
        Ok(())
    }

    pub async fn up(&self, ctx: &SerenityContext, object_id: &u64) -> Result<(), ErrType> {
        self.messages.get(object_id)
            .ok_or(Error::ObjectNotFound(
                format!("Écrit {object_id} non trouvé dans Affichan {} (id: {})",
                        self.chan.get()?.name, self.chan.get()?.id)))?
            .delete(ctx).await?;
        Ok(())
    }

    pub async fn remove(&mut self, ctx: &SerenityContext, object_id: &u64) -> Result<(), ErrType> {
        self.messages.get(object_id).
            ok_or(Error::ObjectNotFound(
            format!("Écrit {object_id} non trouvé dans Affichan {} (id: {})",
                    self.chan.get()?.name, self.chan.get()?.id)))?
            .delete(ctx).await?;
        self.messages.remove(object_id);
        Ok(())
    }

    pub async fn edit_all(&mut self, bot: &Bot<T>, ctx: &SerenityContext) -> Result<(), ErrType> {
        try_join_all(
            self.messages.iter_mut().filter_map(|(object_id, message)| bot.database.get(object_id)
                .map_or_else(|| None, |object| Some((object, message))))
            .map(|(object, message)| message.edit(ctx, object.get_message_edit()))
        ).await?;
        Ok(())
    }

    pub fn contains_object(&self, object_id: &u64) -> bool {
        self.messages.contains_key(object_id)
    }

}