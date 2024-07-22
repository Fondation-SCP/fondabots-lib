use std::collections::{HashMap, HashSet};

use poise::{Context, CreateReply};
use serenity::all::{CreateEmbed, CreateEmbedAuthor, Timestamp};

use crate::{DataType, ErrType, tools};
use crate::object::Field;
use crate::object::Object;
use crate::tools::get_object;

pub fn gcom<T: Object, F: Fn() -> poise::Command<DataType<T>, ErrType>>(
    f: &F,
    name: &str,
    description: &str,
    args: Vec<(&str, &str)>
) -> poise::Command<DataType<T>, ErrType> {
    let mut com = f();
    com.name = name.to_string();
    com.description = Some(description.to_string());
    if args.len() != com.parameters.len() {
        panic!("Erreur : le nombre d’arguments donnés pour la commande {name} ne correspond pas au nombre d’arguments réel.");
    }
    for i in 0..args.len() {
        com.parameters[i].name = args[i].0.to_string();
        com.parameters[i].description = Some(args[i].1.to_string());
    }
    com
}

fn _lister_aux<'a, T: Object, E: Field<T>>(database: &'a HashMap<u64, T>, field: &Option<E>) -> HashSet<&'a u64> {
    let mut res = Vec::new();
    for e in database {
        if E::comply_with(e.1, field) {
            res.push(e);
        }
    }
    res = tools::sort_by_date(res);
    res.into_iter().map(|(id, _) | {id}).collect()
}





/// Liste tous les objets d’après deux propriétés.
#[poise::command(slash_command)]
pub async fn lister_two<T: Object, E1: Field<T>, E2: Field<T>>(
    ctx: Context<'_, DataType<T>, ErrType>,
    field1: Option<E1>,
    field2: Option<E2>
) -> Result<(), ErrType> {
    if field1.is_none() && field2.is_none() {
        Err(ErrType::CommandUseError("au moins l’un des deux paramètres doit être spécifié.".to_string()))?;
    }
    let mut messages = Vec::new();
    let mut buffer = String::new();
    let bot = &mut ctx.data().lock().await;
    let database = &bot.database;


    for objet_id in _lister_aux(database, &field1).intersection(&_lister_aux(database, &field2)) {
        let objet = database.get(objet_id).unwrap();
        let to_add = objet.get_list_entry();
        if buffer.len() + to_add.len() > 1000 {
            messages.push(buffer);
            buffer = String::new();
        }
        buffer += to_add.as_str();
    }
    if !buffer.is_empty() {
        messages.push(buffer);
    }

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

#[poise::command(slash_command)]
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