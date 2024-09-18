use errors::Error;
use serenity::all::Context as SerenityContext;
use serenity::all::Message;
use serenity::all::{ChannelId, MessageId, UserId};
use serenity::futures::future::{join_all, try_join_all};
use std::collections::HashMap;
use tools::PreloadedChannel;
use yaml_rust2::{yaml, Yaml};

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

    pub fn save(&self) -> Yaml {
        Yaml::Array(self.messages.iter().map(|(&object_id, message)| {
            let mut out = yaml::Hash::new();
            out.insert(Yaml::String("id".to_string()), Yaml::Integer(object_id as i64));
            out.insert(Yaml::String("message_id".to_string()), Yaml::Integer(message.id.get() as i64));
            Yaml::Hash(out)
        }).collect())
    }

    pub async fn init(&mut self, database: &HashMap<u64, T>, self_id: &UserId, saved_data: Option<&Yaml>, ctx: &SerenityContext) -> Result<(), ErrType> {
        self.load(ctx).await?;

        match saved_data {
            Some(saved_data) => {
                self.messages = try_join_all(saved_data.as_vec().ok_or(ErrType::YamlParseError("Erreur de yaml dans les affichans: pas un tableau.".to_string()))?
                    .into_iter().map(|yaml_message| async { match yaml_message.as_hash() {
                    Some(_) => {
                        let object_id = yaml_message["id"].as_i64();
                        let message_id = yaml_message["message_id"].as_i64();
                        if object_id.is_none() || message_id.is_none() {
                            Err(ErrType::YamlParseError("Erreur de yaml dans un affichan: un identifiant n’est pas un entier.".into()))
                        } else {
                            match self.chan.get().unwrap().message(ctx, MessageId::new(message_id.unwrap() as u64)).await {
                                Ok(message) => Ok((object_id.unwrap() as u64, message)),
                                Err(e) => Err(ErrType::LibError(Box::new(e)))
                            }
                        }
                    },
                    None => Err(ErrType::YamlParseError("Erreur de yaml dans un affichan: l’une des entrées n’est pas un dictionnaire.".into()))
                }}
                )).await?.into_iter().collect();
            }
            None => {
                let messages = tools::get_channel_messages(self.chan.get()?, ctx, None).await?;
                let self_messages = &self.messages;


                try_join_all(messages.iter().filter(|message|
                    message.author.id.get() == self_id.get()
                        && !message.embeds.is_empty()
                )
                    .filter_map(|message| message.embeds.get(0).unwrap().footer.as_ref().and_then(|footer| Some((message, footer))))
                    .filter_map(|(message, footer)| footer.text.parse().ok().and_then(|footer_text| Some((message, footer_text))))
                    .map(|(message, footer_text)| async move {
                        if let Some(object) = database.get(&footer_text) {
                            if !self_messages.contains_key(&object.get_id()) {
                                Ok(Some((object.get_id(), message.clone())))
                            } else {
                                eprintln!("Message {} en trop: suppression.", message.id);
                                let res = message.delete(ctx).await;
                                res.and_then(|_| Ok(None))
                            }
                        } else {
                            eprintln!("Message {} sans objet associé: message supprimé.", message.id);
                            let res = message.delete(ctx).await;
                            res.and_then(|_| Ok(None))
                        }
                    })).await?
                    .into_iter().filter_map(|option| option)
                    .for_each(|(object_id, message)| {self.messages.insert(object_id, message);});
            }
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

    pub fn get_chan_id(&self) -> u64 {
        match &self.chan {
            PreloadedChannel::Loaded(guild_channel) => &guild_channel.id,
            PreloadedChannel::Unloaded(channel_id) => channel_id
        }.get()
    }

}