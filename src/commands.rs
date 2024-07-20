use poise::{CreateReply, serenity_prelude as serenity};
use poise::Command;
use poise::Context;
use serenity::all::{CreateEmbed, CreateEmbedAuthor, Timestamp};
use serenity::all::CreateAttachment;

use crate::tools::alias;
use crate::tools::get_object;

use super::Bot;
use super::DataType;
use super::ErrType;
use super::Object;

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
#[poise::command(slash_command, category = "Recherche")]
pub async fn rechercher<T: Object>(
    ctx: Context<'_, DataType<T>, ErrType>,
    #[description = "Critère de recherche"] critere: String
) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    let res = bot.search(critere.as_str());
    if res.len() <= 3 && !res.is_empty() {
        ctx.defer().await?;
        for id in res {
            let object = bot.database.get(&id).unwrap();
            ctx.send(object.get_reply()).await?;
        }
    } else if res.is_empty() {
        ctx.send(CreateReply::default().embed(aucun_resultat(critere.as_str()))).await?;
    } else {
        let mut messages = Vec::new();
        let mut buffer = String::new();
        for id in res {
            let object = bot.database.get(&id).unwrap();
            let to_add = object.get_list_entry();
            if buffer.len() + to_add.len() > 1000 {
                messages.push(buffer);
                buffer = String::new();
            }
            buffer += to_add.as_str();
        }
        messages.push(buffer);
        bot.send_embed(&ctx, Bot::<T>::get_multimessages(messages, CreateEmbed::new()
            .title("Résultatss de la recherche")
            .author(CreateEmbedAuthor::new(format!("Recherche : {critere}")))
            .timestamp(Timestamp::now())
            .color(73887))).await?;
    }
    Ok(())
}

/// Commande de test pour vérifier que le bot fonctionne.
#[poise::command(slash_command, category = "Salons d’affichage")]
pub async fn plop<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.send(CreateReply::default().content("Plop !")).await?;
    Ok(())
}

/// Supprime un objet de la base de données.
///
/// Le nom entré doit être suffisamment précis pour identifier un seul écrit. Sinon, entrer
/// l’identifiant de l’écrit et mettre est_id à true. Attention : il n’y a pas de confirmation,
/// soyez sûr d’entrer le bon nom.
#[poise::command(slash_command, category = "Édition")]
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
#[poise::command(slash_command, category = "Édition")]
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
#[poise::command(slash_command, category = "Salons d’affichage")]
pub async fn update_affichans<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    ctx.data().lock().await.update_affichans(ctx.serenity_context()).await?;
    ctx.send(CreateReply::default().content("Affichans mis à jour.")).await?;
    Ok(())
}

/// Renomme un objet.
#[poise::command(slash_command, category = "Édition")]
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
#[poise::command(slash_command, category = "Entretien de la base de données")]
pub async fn doublons<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    let bot = &mut ctx.data().lock().await;
    let mut noms_presents = Vec::new();
    let mut id_to_delete = Vec::new();
    for (id, object) in &bot.database {
        if noms_presents.contains(&object.get_name()) {
            id_to_delete.push(*id);
        } else {
            noms_presents.push(object.get_name());
        }
    }
    let nb_deleted = id_to_delete.len();
    bot.archive(id_to_delete.clone());
    for id in id_to_delete {
        bot.database.remove(&id);
    }
    ctx.send(CreateReply::default()
        .content(if nb_deleted == 0 {
            format!("Aucun doublon trouvé.")
        } else {
            let pluriel = if nb_deleted == 1 {"s"} else {""};
            format!("{} doublon{pluriel} supprimé{pluriel}.", nb_deleted)
        })).await?;
    Ok(())
}

/// Remet un objet à l’avant des salons d’affichage
#[poise::command(slash_command, category = "Salons d’affichage")]
pub async fn up<T: Object>(ctx: Context<'_, DataType<T>, ErrType>,
    #[description = "Critère d’identification de l’objet."] critere: String) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    if let Some(object_id) = get_object(&ctx, bot, &critere).await? {
        for affichan in &bot.affichans {
            match affichan.up(ctx.serenity_context(), &object_id).await {
                Err(ErrType::ObjectNotFound(_)) | Ok(_) => (), /* Osef si l’objet n’est pas dans un des affichans */
                error => return error
            }
        }
        bot.archive(vec![object_id]);
        bot.database.get_mut(&object_id).unwrap().up();
        ctx.say(format!("Objet {} up !", bot.database.get(&object_id).unwrap().get_name())).await?;
        bot.update_affichans(ctx.serenity_context()).await?;
    }
    Ok(())
}

/// Réinitialise les salons d’affichage.
#[poise::command(slash_command, category = "Salons d’affichage")]
pub async fn refresh_affichans<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    let bot = &mut ctx.data().lock().await;
    ctx.defer().await?;
    for affichan in &mut bot.affichans {
        affichan.refresh(ctx.serenity_context()).await?;
    }
    ctx.say("Salons d’affichage réinitialisés.").await?;
    Ok(())
}

/// Renvoie la base de données.
#[poise::command(slash_command, category = "Base de données")]
pub async fn bdd<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    ctx.send(CreateReply::default().attachment(CreateAttachment::path(&ctx.data().lock().await.data_file).await?)).await?;
    Ok(())
}

/// Renvoie le nombre d’objets dans la base de données.
#[poise::command(slash_command, category = "Base de données")]
pub async fn taille_bdd<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.send(CreateReply::default().content(
        format!("Il y a actuellement {} écrits dans la base de données.",
            ctx.data().lock().await.database.len())
    )).await?;
    Ok(())
}

/// Sauvegarde la base de données.
#[poise::command(slash_command, category = "Base de données")]
pub async fn save<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> Result<(), ErrType> {
    ctx.defer().await?;
    ctx.data().lock().await.save()?;
    ctx.say("Base de données sauvegardée !").await?;
    Ok(())
}

/// Appelle manuellement la commande de mise à jour RSS. Modification non annulable.
#[poise::command(slash_command, category = "Base de données")]
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




pub fn command_list<T: Object>() -> Vec<Command<DataType<T>, ErrType>> {
    vec![rechercher(), plop(), supprimer(), annuler(), update_affichans(), renommer(), doublons(),
         up(), refresh_affichans(), bdd(), taille_bdd(), save(), maj(),
        alias("search", rechercher())]
}