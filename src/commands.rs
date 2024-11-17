//! Ce module contient les commandes présentes par défaut avec les bots construits avec cette librairie.

use super::tools;
use super::DataType;
use super::ErrType;
use super::Object;
use crate::command_data::{CommandData, Permission};
use crate::tools::alias;
use crate::tools::get_object;
use poise::Command;
use poise::Context;
use poise::{serenity_prelude as serenity, CreateReply};
use serenity::all::CreateAttachment;
use serenity::all::{CreateEmbed, CreateEmbedAuthor, Timestamp};
use serenity::futures::future::try_join_all;

/// Renvoie l’embed « Aucun résultat » en indiquant la recherche de l’utilisateur.
pub fn aucun_resultat(recherche: &str) -> CreateEmbed {
    CreateEmbed::new()
        .title("Aucun résultat.")
        .color(16001600)
        .author(CreateEmbedAuthor::new(format!("Recherche : {}", recherche)))
        .timestamp(Timestamp::now())
}

/// Recherche des objets par nom.
///
/// Cette commande affiche tous les objets contenant le critère demandé.
/// Un objet sera affiché dans les résultats s’il contient chaque mot du critère dans
/// les mots de son nom.
#[poise::command(slash_command, category = "Recherche", custom_data = CommandData::perms(Permission::READ), check = CommandData::check)]
pub async fn rechercher<T: Object>(
    ctx: Context<'_, DataType<T>, ErrType>,
    #[description = "Critère de recherche"] critere: String
) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    let res = bot.search(critere.as_str());
    if res.len() <= 3 && !res.is_empty() {
        ctx.defer().await?;
        try_join_all(
            res.into_iter().map(|id| ctx.send(bot.database.get(&id).unwrap().get_reply()))
        ).await?;
    } else if res.is_empty() {
        ctx.send(CreateReply::default().embed(aucun_resultat(critere.as_str()))).await?;
    } else {
        let messages = tools::create_paged_list(res, |id|
            bot.database.get(id).unwrap().get_list_entry(),
        1000);
        bot.send_embed(&ctx, tools::get_multimessages(messages, CreateEmbed::new()
            .title("Résultatss de la recherche")
            .author(CreateEmbedAuthor::new(format!("Recherche : {critere}")))
            .timestamp(Timestamp::now())
            .color(73887))).await?;
    }
    Ok(())
}

/// Commande de test pour vérifier que le bot fonctionne.
#[poise::command(slash_command, category = "Salons d’affichage", custom_data = CommandData::perms(Permission::READ), check = CommandData::check)]
pub async fn plop<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.send(CreateReply::default().content("Plop !")).await?;
    Ok(())
}

/// Supprime un objet de la base de données.
///
/// Le nom entré doit être suffisamment précis pour identifier un seul écrit. Sinon, entrer
/// l’identifiant de l’écrit et mettre est_id à true. Attention : il n’y a pas de confirmation,
/// soyez sûr d’entrer le bon nom.
#[poise::command(slash_command, category = "Édition", custom_data = CommandData::perms(Permission::WRITE), check = CommandData::check)]
pub async fn supprimer<T: Object>(ctx: Context<'_, DataType<T>, ErrType>,
    #[description = "Critère d’identification de l’objet"] critere: String) -> Result<(), ErrType> {

    let bot = &mut ctx.data().lock().await;
    if let Some(object_id) = get_object(&ctx, bot, &critere).await? {
        bot.archive(vec![object_id]);
        ctx.send(CreateReply::default()
            .content(format!("Objet « {} » supprimé.",
                             bot.database.remove(&object_id).unwrap().get_name()))).await?;
        bot.update_affichans(ctx.serenity_context()).await?;
    }
    Ok(())
}

/// Annule la dernière action effectuée sur la base de données.
#[poise::command(slash_command, category = "Édition", custom_data = CommandData::perms(Permission::WRITE), check = CommandData::check)]
pub async fn annuler<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    if bot.annuler() {
        ctx.send(CreateReply::default().content("Dernière modification annulée !")).await?;
    } else {
        ctx.send(CreateReply::default().content("Aucune modification récente annulable.")).await?;
    }
    Ok(())
}

/// Vérifie que les salons d’affichage sont bien à jour.
#[poise::command(slash_command, category = "Salons d’affichage", custom_data = CommandData::perms(Permission::MANAGE), check = CommandData::check)]
pub async fn update_affichans<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    ctx.data().lock().await.update_affichans(ctx.serenity_context()).await?;
    ctx.send(CreateReply::default().content("Affichans mis à jour.")).await?;
    Ok(())
}

/// Renomme un objet.
#[poise::command(slash_command, category = "Édition", custom_data = CommandData::perms(Permission::WRITE), check = CommandData::check)]
pub async fn renommer<T: Object>(ctx: Context<'_, DataType<T>, ErrType>,
    #[description = "Critère d’identification de l’objet"] critere: String,
    #[description = "Nouveau nom de l’objet"] nouveau_nom: String) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    if let Some(object_id) = get_object(&ctx, bot, &critere).await? {
        bot.archive(vec![object_id]);
        ctx.send(CreateReply::default().content(format!("Écrit {} renommé en {nouveau_nom} !",
            bot.database.get(&object_id).unwrap().get_name()))).await?;
        bot.database.get_mut(&object_id).unwrap().set_name(nouveau_nom);
    }

    Ok(())
}

/// Supprime les doublons de la base de données.
#[poise::command(slash_command, category = "Entretien de la base de données", custom_data = CommandData::perms(Permission::MANAGE), check = CommandData::check)]
pub async fn doublons<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    let bot = &mut ctx.data().lock().await;
    let database = &mut bot.database;
    let (_, doublons) = database.iter().fold((Vec::new(), Vec::new()), |(names, to_del), (object_id, object)| {
        if names.contains(&object.get_name()) {
            (names, vec![to_del, vec![*object_id]].concat())
        } else {
            (vec![names, vec![object.get_name()]].concat(), to_del)
        }
    });
    let nb_deleted = doublons.len();
    bot.archive(doublons.clone());
    /* Réemprunt après archive */
    let database = &mut bot.database;
    doublons.iter().for_each(|doublon| {database.remove(&doublon);});

    ctx.send(CreateReply::default()
        .content(if nb_deleted == 0 {
            "Aucun doublon trouvé.".to_string()
        } else {
            let pluriel = if nb_deleted == 1 {"s"} else {""};
            format!("{} doublon{pluriel} supprimé{pluriel}.", nb_deleted)
        })).await?;
    Ok(())
}

/// Remet un objet à l’avant des salons d’affichage
#[poise::command(slash_command, category = "Salons d’affichage", custom_data = CommandData::perms(Permission::WRITE), check = CommandData::check)]
pub async fn up<T: Object>(ctx: Context<'_, DataType<T>, ErrType>,
    #[description = "Critère d’identification de l’objet."] critere: String) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    if let Some(object_id) = get_object(&ctx, bot, &critere).await? {
        try_join_all(bot.affichans.iter()
            .filter(|affichan| affichan.contains_object(&object_id))
            .map(|affichan| affichan.up(ctx.serenity_context(), &object_id))
        ).await?;
        bot.archive(vec![object_id]);
        bot.database.get_mut(&object_id).unwrap().up();
        ctx.say(format!("Objet {} up !", bot.database.get(&object_id).unwrap().get_name())).await?;
        bot.update_affichans(ctx.serenity_context()).await?;
    }
    Ok(())
}

/// Réinitialise les messages des salons d’affichage.
#[poise::command(slash_command, category = "Salons d’affichage", custom_data = CommandData::perms(Permission::MANAGE), check = CommandData::check)]
pub async fn refresh_affichans<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    ctx.defer().await?;
    try_join_all(bot.affichans.iter_mut().map(|affichan| affichan.refresh(ctx.serenity_context()))).await?;
    ctx.say("Messages des salons d’affichage réinitialisés.").await?;
    Ok(())
}

/// Réinitialise les affichans
#[poise::command(slash_command, category = "Salons d’affichage", custom_data = CommandData::perms(Permission::MANAGE), check = CommandData::check)]
pub async fn reset_affichans<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    ctx.defer().await?;
    try_join_all(bot.affichans.iter_mut().map(|affichan| affichan.purge(ctx.serenity_context()))).await?;
    bot.update_affichans(ctx.serenity_context()).await?;
    ctx.say("Salons d’affichage réinitialisés.").await?;
    Ok(())
}

/// Renvoie la base de données.
#[poise::command(slash_command, category = "Base de données", custom_data = CommandData::perms(Permission::MANAGE), check = CommandData::check)]
pub async fn bdd<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    ctx.send(CreateReply::default().attachment(CreateAttachment::path(&ctx.data().lock().await.data_file).await?)).await?;
    Ok(())
}

/// Renvoie le nombre d’objets dans la base de données.
#[poise::command(slash_command, category = "Base de données", custom_data = CommandData::perms(Permission::READ), check = CommandData::check)]
pub async fn taille_bdd<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.send(CreateReply::default().content(
        format!("Il y a actuellement {} écrits dans la base de données.",
            ctx.data().lock().await.database.len())
    )).await?;
    Ok(())
}

/// Sauvegarde la base de données.
#[poise::command(slash_command, category = "Base de données", custom_data = CommandData::perms(Permission::READ), check = CommandData::check)]
pub async fn save<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    ctx.data().lock().await.save()?;
    ctx.say("Base de données sauvegardée !").await?;
    Ok(())
}

/// Appelle manuellement la commande de mise à jour RSS. Modification non annulable.
#[poise::command(slash_command, category = "Base de données", custom_data = CommandData::perms(Permission::MANAGE), check = CommandData::check)]
pub async fn maj<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    let taille_ancienne = ctx.data().lock().await.database.len();
    T::maj_rss(ctx.data()).await?;
    if taille_ancienne != ctx.data().lock().await.database.len() {
        ctx.data().lock().await.update_affichans(&ctx.serenity_context()).await?;
    }
    ctx.say("Mise à jour effectuée !").await?;
    Ok(())
}


/// Cette commande supprime tous les enregistrements des commandes Discord et éteint le bot.
///
/// Elle n'est accessible qu'à la personne qui gère le bot actuellement; cette fonctionnalité
/// n'étant pas encore implémentée, elle n'est accessible à personne.
#[poise::command(slash_command, owners_only)]
pub async fn delete_commands<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    let serenity_ctx = ctx.serenity_context();
    ctx.defer().await?;
    // Fetch global commands
    let global_commands = serenity_ctx.http.get_global_commands().await?;

    try_join_all(global_commands.into_iter().map(|command| {
        println!("Suppression de la commande {}", command.name);
        serenity_ctx.http.delete_global_command(command.id)
    })).await?;

    // Fetch the guilds the bot is a part of
    let guilds = serenity_ctx.http.get_guilds(None, None).await?;
    try_join_all(
        try_join_all(
            guilds.into_iter().map(|guild| serenity_ctx.http.get_guild_commands(guild.id))
        ).await?.concat().into_iter().map(|guild_command| {
            println!("Suppression de la commande de serveur {}", guild_command.name);
            serenity_ctx.http.delete_guild_command(guild_command.guild_id.unwrap(), guild_command.id)
        })
    ).await?;

    ctx.say("Commandes du bot supprimées. Le bot va désormais s’éteindre.").await?;
    panic!("Commande delete_commands terminée.")
}



/// Enregistrement des commandes par défaut de la bibliothèque fondabots.
pub fn command_list<T: Object>() -> Vec<Command<DataType<T>, ErrType>> {
    vec![rechercher(), plop(), supprimer(), annuler(), update_affichans(), renommer(), doublons(),
         up(), refresh_affichans(), bdd(), taille_bdd(), save(), maj(),
        alias("search", rechercher()), delete_commands(), reset_affichans()]
}