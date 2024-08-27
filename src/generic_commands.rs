use std::collections::{HashMap, HashSet};

use poise::{Context, CreateReply};
use serenity::all::{CreateEmbed, CreateEmbedAuthor, Timestamp};

use crate::object::Field;
use crate::object::Object;
use crate::tools::get_object;
use crate::{tools, DataType, ErrType};

fn _lister_one<'a, T: Object, E: Field<T>>(database: &'a HashMap<u64, T>, field: &Option<E>) -> HashSet<&'a u64> {
    tools::sort_by_date(database.iter().filter(|(_, object)| E::comply_with(object, field)).collect())
        .into_iter().map(|(id, _) | {id}).collect()
}

pub async fn lister_two<T: Object, E1: Field<T>, E2: Field<T>>(
    ctx: Context<'_, DataType<T>, ErrType>,
    field1: Option<E1>,
    field2: Option<E2>
) -> Result<(), ErrType> {
    if field1.is_none() && field2.is_none() {
        Err(ErrType::CommandUseError("au moins l’un des deux paramètres doit être spécifié.".to_string()))?;
    }
    let bot = &mut ctx.data().lock().await;
    let database = &bot.database;

    let messages = tools::create_paged_list(
        _lister_one(database, &field1).intersection(&_lister_one(database, &field2)).collect(),
        |object| database.get(object).unwrap().get_list_entry(),
        1000
    );

    if messages.is_empty() {
        ctx.send(CreateReply::default().embed(CreateEmbed::new()
            .title("Aucun résultat.")
            .color(16001600)
            .author(CreateEmbedAuthor::new(format!("Recherche : {} – {}",
                                                   if let Some(s) = field1 {s.to_string()} else {"Tous".to_string()},
                                                   if let Some(t) = field2 {t.to_string()} else {"Tous".to_string()})))
            .timestamp(Timestamp::now()))).await?;
    } else {
        bot.send_embed(&ctx, tools::get_multimessages(messages, CreateEmbed::new()
            .author(CreateEmbedAuthor::new(format!("Recherche : {} – {}",
                                                   if let Some(s) = field1 {s.to_string()} else {"Tous".to_string()},
                                                   if let Some(t) = field2 {t.to_string()} else {"Tous".to_string()}
            )))
            .title("Résultats de la recherche")
            .timestamp(Timestamp::now())
            .color(73887))).await?;
    }

    Ok(())
}


pub async fn change_field<T: Object, F: Field<T>>(ctx: Context<'_, DataType<T>, ErrType>,
                    critere: String,
                    field: F) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    if let Some(object_id) = get_object(&ctx, bot, &critere).await? {
        bot.archive(vec![object_id]);
        let object = bot.database.get_mut(&object_id).unwrap();
        ctx.say(format!("{} de « {} » changé pour « {field} »", F::field_name() ,object.get_name())).await?;
        F::set_for(object, &field);
        object.set_modified(true);
    }
    Ok(())
}