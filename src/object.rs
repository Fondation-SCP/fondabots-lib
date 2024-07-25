use std::fmt::{Debug, Display};
use std::str::FromStr;

use poise::{ChoiceParameter, serenity_prelude as serenity};
use poise::CreateReply;
use serenity::{ComponentInteraction, CreateActionRow, CreateEmbed, CreateMessage, EditMessage, Timestamp};
use serenity::all::ArgumentConvert;
use serenity::Context as SerenityContext;
use yaml_rust2::Yaml;

use crate::{Bot, DataType, ErrType};

pub trait Object: Send + Sync + 'static + PartialEq + Clone + Debug {
    #[deprecated(since = "1.1.0", note = "Méthode inutile. Sera supprimée en 2.0.0")]
    fn new() -> Self {
        unimplemented!("Méthode supprimée.")
    }
    fn get_id(&self) -> u64;
    fn from_yaml(data: &Yaml) -> Result<Self, ErrType>;
    fn serialize(&self) -> Yaml;
    fn is_modified(&self) -> bool;
    fn set_modified(&mut self, modified: bool);
    fn get_embed(&self) -> CreateEmbed;
    fn get_buttons(&self) -> CreateActionRow;
    fn get_message(&self) -> CreateMessage {
        CreateMessage::new().embed(self.get_embed()).components(vec![self.get_buttons()])
    }
    fn get_message_edit(&self) -> EditMessage {
        EditMessage::new().embed(self.get_embed()).components(vec![self.get_buttons()])
    }
    fn get_reply(&self) -> CreateReply {
        CreateReply::default().embed(self.get_embed()).components(vec![self.get_buttons()])
    }
    fn get_name(&self) -> &String;
    fn set_name(&mut self, s: String);
    fn get_list_entry(&self) -> String;

    fn up(&mut self);

    fn buttons(ctx: &SerenityContext, interaction: &mut ComponentInteraction, bot: &mut Bot<Self>) -> impl std::future::Future<Output = Result<(), ErrType>> + Send;

    fn maj_rss(bot: &DataType<Self>) -> impl std::future::Future<Output = Result<(), ErrType>> + Send;

    fn get_date(&self) -> &Timestamp {
        unimplemented!("Cette méthode devrait être ré-implémentée.")
    }

    fn set_date(&mut self, _t: Timestamp) {
        unimplemented!("Cette méthode devrait être ré-implémentée.")
    }
}

pub trait Field<T: Object>: Eq + ChoiceParameter + Display + Clone + Sync + ArgumentConvert + Send + FromStr {
    fn comply_with(obj: &T, field: &Option<Self>) -> bool;
    fn set_for(obj: &mut T, field: &Self);
    fn field_name() -> &'static str;
}