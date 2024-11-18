//! Module contenant de nombreuses fonctions auxiliaires prenant en charge des commandes pouvant
//! s’adapter selon le type d’objet créé. Ces commandes ne peuvent cependant pas être entièrement
//! crées de manière générique car le nombre de [`Field`] différents présents dans un [`Object`]
//! est totalement au désir de l’utilisateur. Il peut alors exister plusieurs commandes différentes
//! utilisant les mêmes fonctions auxiliaires pour des [`Field`] différents.
//!
//! Pour créer une commande [`poise`] utilisant une fonction auxiliaire de ce module, il suffit
//! de créer la commande normalement avec les paramètres et leur description, puis d’appeler
//! la fontion auxiliaire dans le corps de la commande. Celle-ci effectue alors directement
//! la commande avec les [`Field`] donnés, et s’occupe de tout (notamment l’affichage des résultats).

use std::collections::{HashMap, HashSet};

use poise::{Context, CreateReply};
use serenity::all::{CreateEmbed, CreateEmbedAuthor, Timestamp};

use crate::object::Field;
use crate::object::Object;
use crate::tools::get_object;
use crate::{tools, DataType, ErrType};

/* Fonction auxiliaire renvoyant tous les objets ayant le champ demandé à la valeur demandée */
fn _lister_one<'a, T: Object, E: Field<T>>(database: &'a HashMap<u64, T>, field: &Option<E>) -> HashSet<&'a u64> {
    tools::sort_by_date(database.iter().filter(|(_, object)| E::comply_with(object, field)).collect())
        .into_iter().map(|(id, _) | {id}).collect()
}

/// Auxiliaire générique pour une commande lister à deux champs. Effectue une recherche parmi la
/// base de données et affiche les résultats. Si l’une des entrées est définie à [`None`], alors
/// la recherche acceptera tout type de champs.
///
/// La commande échoue (message d’erreur Discord) si les deux champs sont [`None`].
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

/// Fonction auxiliaire permettant la modification d’un champ [`Field`] donné.
pub async fn change_field<T: Object, F: Field<T>>(ctx: Context<'_, DataType<T>, ErrType>,
                    critere: String,
                    field: F) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    if let Some(object_id) = get_object(&ctx, bot, &critere).await? {
        bot.archive(vec![object_id]);
        let object = bot.database.get(&object_id).unwrap();
        ctx.say(format!("{} de « {} » changé pour « {field} »", F::field_name() ,object.get_name())).await?;
        bot.log(&ctx, format!("{} a changé la propriété {} de l'objet {} (id: {}) pour {}.",
            tools::user_desc(ctx.author()),
            F::field_name(),
            object.get_name(),
            object_id,
            field
        )).await?;
        let object = bot.database.get_mut(&object_id).unwrap();
        F::set_for(object, &field);
        object.set_modified(true);
    }
    Ok(())
}