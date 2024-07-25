use std::collections::HashMap;

use serenity::all::{ChannelId, GetMessages, MessageId, UserId};
use serenity::all::Context as SerenityContext;
use serenity::all::Message;

use errors::Error;
use tools::PreloadedChannel;

use crate::{errors, Object, tools};
use crate::Bot;
use crate::ErrType;
use crate::tools::Preloaded;

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

    pub async fn init(&mut self, database: &mut HashMap<u64, T>, self_id: &UserId, ctx: &SerenityContext) -> Result<(), ErrType> {
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

    pub async fn update(&mut self, database: &mut HashMap<u64, T>, ctx: &SerenityContext) -> Result<(), ErrType> {
        let mut to_del = Vec::new();
        for (object_id, message) in &mut self.messages {
            if if !(database.contains_key(object_id) && (self.test)(database.get(object_id))) {
                to_del.push(object_id.clone());
                message.delete(ctx).await

            } else if database.get(object_id).unwrap() /* Error impossible due to previous condition */.is_modified() {
                message.edit(ctx, database.get(object_id).unwrap().get_message_edit()).await
            } else {
                Ok(())
            }.is_err() {
                eprintln!("Message {} invalide trouvé dans l’affichan {}. Suppression.", message.id.get(), self.chan.get()?.name);
                to_del.push(object_id.clone());
            }
        }

        for del in to_del {
            self.messages.remove(&del);
        }

        for (&object_id, object) in tools::sort_by_date(database.iter()
            .filter(| (id, obj) | {(self.test)(Some(obj)) && !self.messages.contains_key(id)}).collect()).into_iter().rev() {
                self.messages.insert(object_id, self.chan.get()?.send_message(ctx, object.get_message()).await?);
        }
        Ok(())
    }

    pub async fn purge(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        self.refresh(ctx).await?;
        self.messages.clear();
        Ok(())
    }

    pub async fn refresh(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        for (_, message) in &mut self.messages {
            message.delete(ctx).await?;
        }
        Ok(())
    }

    pub async fn check_message_deletion(&self, bot: &Bot<T>, ctx: &SerenityContext, message_id: &MessageId) -> Result<(), ErrType> {
        for (object_id, message) in &self.messages {
            if message.id.get() == message_id.get() {
                self.chan.get()?.send_message(ctx, bot.database.get(object_id)
                    .ok_or(Error::ObjectNotFound(format!("Objet {object_id} référencé dans un message supprimé dans Affichan {} (id: {})",
                        self.chan.get()?.name, self.chan.get()?.id)))?
                    .get_message()).await?;
                break; /* Seul un message identique peut exister */
            }
        }
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
        for (object_id, message) in &mut self.messages {
            if let Some(object) = bot.database.get(object_id) {
                message.edit(ctx, object.get_message_edit()).await?;
            } else {
                eprintln!("Écrit {object_id} présent dans Affichan {} (id: {}) mais absent de la base de données.",
                    self.chan.get()?.name, self.chan.get()?.id);
                continue;
            }
        }
        Ok(())
    }

}