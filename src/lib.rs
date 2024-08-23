//! Bibliothèque partagée pour les bots Discord de la Fondation SCP.
//!
//! Ces bots ont pour fonctionnalité de récupérer des éléments (fils à critiquer, fils staff, pages
//! à relire…) afin de les afficher dans un salon et de les gérer par des commandes (recherche,
//! liste par caractéristique…) et des boutons directement dans le salon d’affichage.
//!
//! Cette bibliothèque inclut également une fonctionnalité de sauvegarde de la base de données au
//! format YAML.
//!
//! ### Utilisation
//! Pour utiliser cette bibliothèque, il faut :
//! * Définir une structure d’objet implémentant [`object::Object`]
//! * Définir des caractéristiques qui seront des champs de la structure définie plus haut,
//! implémentant [`object::Field`]
//! * Définir éventuellement des commandes supplémentaires dont la liste est à donner au bot.
//!
//! ### Exemples
//! Deux exemples principaux d’implémentation de la bibliothèque sont disponibles :
//! * [Critibot](https://github.com/Fondation-SCP/critibot) – Le bot Discord qui sert à organiser les critiques
//! * [Staffbot](https://github.com/Fondation-SCP/staffbot) – Le bot Discord qui sert à organiser
//! les fils du [site staff](https://commandementO5.wikidot.com/).
//!

//#![deny(missing_docs)]
#![doc(issue_tracker_base_url = "https://github.com/Fondation-SCP/fondabots-lib/issues/")]


use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use poise::reply::CreateReply;
use poise::Context;
use poise::{serenity_prelude as serenity, Framework};
use serenity::all::UserId;
use serenity::all::{ButtonStyle, Context as SerenityContext, CreateInteractionResponse, CreateInteractionResponseMessage, GuildChannel, MessageId};
use serenity::all::{ComponentInteraction, CreateButton, GatewayIntents};
use serenity::all::{CreateActionRow, EditMessage, Interaction};
use serenity::client::ClientBuilder;
use serenity::prelude::*;
use serenity::CreateEmbed;
use serenity::FullEvent;
use tokio::time;
use yaml_rust2::{yaml, Yaml, YamlEmitter, YamlLoader};

use affichan::Affichan;
pub use errors::Error as ErrType;
#[deprecated(since = "1.1.0", note = "Utiliser fondabots_lib::object::Object")]
pub use object::Object;

use crate::tools::basicize;

pub mod affichan;
mod commands;
pub mod errors;
pub mod tools;
pub mod generic_commands;
pub mod object;

/// Redéfinition du type utilisé pour des données de [`poise`], utilisant un [`Arc`] et un [`Mutex`]
/// sur [`Bot`] pour lui permettre d’obtenir une référence mutable dans chaque commande si besoin.
///
/// `T` doit implémenter [`object::Object`] : il faut garder en tête que ce type n’est qu’un
/// raccourci vers [`Bot`] qui impose `T: Object`.
pub type DataType<T> = Arc<Mutex<Bot<T>>>;

/// Structure de données de bot. Cette structure, où `T` est l’implémentation d’un [`object::Object`]
/// pour le bot souhaité, contient, entre autre, la base de données et les salons d’affichage.
pub struct Bot<T: Object> {
    /// Base de données des objets.
    ///
    /// Chaque objet doit avoir un identifiant unique qui sera utilisé comme clé dans la [`HashMap`].
    pub database: HashMap<u64, T>,
    /* Historique utilisé dans Bot::archive et Bot::annuler
        Il prend la forme d’une pile de vecteurs représentant une modification, contenant
        des tuples de chaque objet modifié contenant leur identifiant et une option
        sur les objets. Si cette option est None c’est que l’objet a été crée, et sera donc
        supprimé en cas d’annulation de l’action. */
    history: VecDeque<Vec<(u64, Option<T>)>>,

    /// Date et heure du dernier écrit récupéré dans les flux RSS. Ce champ est à réutiliser dans
    /// [`object::Object::maj_rss`] pour éviter de récupérer plusieurs fois le même écrit.
    pub last_rss_update: DateTime<Utc>,

    /* Identifiant du bot. None si le bot n’est pas encore chargé. */
    self_id: Option<UserId>,

    /* Liste des multimessages. L’identifiant est le timestamp de la création des multimessages. */
    multimessages: HashMap<String, Vec<CreateEmbed>>,

    /* Positions actuelle des multimessages, par la même clé. */
    mmpositions: HashMap<String, usize>,

    /* Salons d’affichage */
    affichans: Vec<Affichan<T>>,

    /* Chemin de fichier vers le fichier de sauvegarde */
    data_file: String,

    /* Stockage des salons absolus, c’est-à-dire des salons accessibles dans toute commande. */
    absolute_chans: HashMap<&'static str, GuildChannel>,

    /// Trigger permettant la mise à jour des salons d’affichage à la fin du traitement de l’évènement.
    ///
    /// Passer à `true` pour activer la mise à jour (appel à [`Bot::update_affichans`]),
    /// repassera à `false` après. Ce trigger permet de delayer cette mise à jour afin de ne pas
    /// bloquer le thread et de ne pas utiliser de `await`.
    pub update_affichans: bool
}

impl<T: Object> Bot<T> {

    /// Création du bot. Attention, une fois le bot crée, il faudra le lancer par un appel à
    /// [`serenity::Client::start`] sur le [`Client`] renvoyé.
    ///
    /// Les salons « absolus » correspondent à des salons accessibles depuis toutes les
    /// commandes, qui sont à fournir par un nom et un identifiant.
    ///
    /// # Panics
    /// Cette méthode essaye au maximum de renvoyer ses erreurs, mais panique en cas d’erreur
    /// dans le chargement du fichier de sauvegarde en YAML pour éviter toute corruption ou
    /// suppression accidentelle de données.
    ///
    pub async fn new(
        token: String,
        intents: GatewayIntents,
        savefile_path: &str,
        mut commands: Vec<poise::Command<DataType<T>, ErrType>>,
        affichans: Vec<Affichan<T>>,
        absolute_chans: HashMap<&'static str, u64>
    ) -> Result<Client, ErrType> {
        println!("Lancement du bot.");
        let data_str = fs::read_to_string(savefile_path);
        let mut last_update = 0;
        let mut bot = Self {
            database: {
                if let Ok(s) = data_str {
                    println!("Chargement des données.");
                    let data = &YamlLoader::load_from_str(s.as_str())?[0];
                    last_update = data["last_rss_update"].as_i64().unwrap_or(0);
                    let mut ret = HashMap::new();
                    let entries = data["entries"].as_vec()
                        .ok_or(ErrType::YamlParseError("Dans les données, entries n’est pas un tableau.".to_string()))?;
                    for entry in entries {
                        match T::from_yaml(entry) {
                            Ok(obj) => {ret.insert(obj.get_id(), obj);}
                            Err(e) => {
                                let mut debug_out = String::new();
                                let mut debug_emitter = YamlEmitter::new(&mut debug_out);
                                debug_emitter.compact(false);
                                debug_emitter.multiline_strings(true);
                                debug_emitter.dump(entry)?;
                                panic!("Erreur de chargement ({e}) dans le yaml suivant: {debug_out}")
                            }
                        }

                    }
                    ret
                } else {
                    println!("Pas de base de donnée trouvée : création d’une nouvelle.");
                    HashMap::new()
                }
            },
            last_rss_update: DateTime::from_timestamp(last_update, 0)
                .ok_or(ErrType::YamlParseError("Mauvais format de date pour last_rss_update.".to_string()))?,
            self_id: None,
            history: VecDeque::new(),
            multimessages: HashMap::new(),
            mmpositions: HashMap::new(),
            affichans,
            data_file: savefile_path.to_string(),
            absolute_chans: HashMap::new(),
            update_affichans: false
        };

        println!("Création du framework.");

        commands.append(&mut commands::command_list());

        let framework = Framework::builder()
            .options(poise::FrameworkOptions {
                commands,
                event_handler: |ctx, event, _framework_context, data| {
                    Box::pin(async move {
                        let bot = &mut data.lock().await;
                        match {if let FullEvent::InteractionCreate { interaction, .. } = event {
                                if let Interaction::Component(ref mut component) = interaction.clone() {
                                    bot.handle_interaction(ctx, component).await
                                } else { Ok(()) }
                            } else if let FullEvent::MessageDelete {deleted_message_id, ..} = event {
                                bot.check_deletions(ctx, &deleted_message_id).await
                            } else {Ok(())}}
                        {
                            Ok(o) => Ok::<(), ErrType>(o),
                            Err(e) => {
                                eprintln!("Erreur lors de la réception d’un évènement : {e}");
                                return Err(e)
                            }
                        }?;
                        if bot.update_affichans {
                            if let Err(e) = bot.update_affichans(ctx).await {
                                eprintln!("Erreur lors de la mise à jour des affichans : {e}");
                                return Err(e);
                            }
                            bot.update_affichans = false;
                        }
                        if let Err(e) = bot.save() {
                            eprintln!("Erreur lors d’une sauvegarde de routine: {e}");
                        }
                        Ok(())

                    })
                },
                ..Default::default()
            })
            .setup(|ctx, ready, framework| {
                Box::pin(async move {
                    println!("Bot connecté à Discord. Réglage des derniers détails.");
                    println!("Enregistrement des commandes.");
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    println!("Récupération de l’identifiant.");
                    bot.self_id = Some(ready.user.id);
                    println!("Chargement des salons d’affichage.");
                    for affichan in &mut bot.affichans {
                        affichan.init(&mut bot.database, &bot.self_id.unwrap(), ctx).await?;
                    }
                    println!("Chargement des salons absolus.");
                    for (name, chan_id) in absolute_chans {
                        match serenity::ChannelId::new(chan_id).to_channel(ctx).await {
                            Ok(chan) => {
                                bot.absolute_chans.insert(name, try_loop!(chan.guild()
                                .ok_or(ErrType::NoneError), "Salon absolu n’est pas un salon de serveur."));
                            },
                            Err(e) => {
                                eprintln!("Erreur dans le chargement du salon absolu {name} : {e}");
                                continue;
                            }
                        }
                        println!("Salon {name} chargé.");

                    }
                    let bot_mutex = Arc::new(Mutex::new(bot));
                    let bot_mutex_2 = bot_mutex.clone();
                    println!("Démarrage du thread RSS.");
                    tokio::spawn(async move {
                        let mut delay = time::interval(Duration::from_secs(600));
                        loop {
                            if let Err(e) = T::maj_rss(&bot_mutex).await {
                                println!("Erreur lors d’une mise à jour RSS: {e}");
                            }
                            delay.tick().await;
                        }
                    });
                    println!("Chargement terminé !");
                    Ok(bot_mutex_2)
                })
            }).build();

        println!("Création du bot terminé, client crée.");

        Ok(ClientBuilder::new(token, intents).framework(framework).await?)
    }

    /// Renvoie une référence vers le salon du nom donné, ou une erreur s’il n’existe pas.
    pub fn get_absolute_chan(&self, name: &'static str) -> Result<&GuildChannel, ErrType> {
        self.absolute_chans.get(name).ok_or(ErrType::ObjectNotFound(format!("Salon absolu {name} inexistant.")))
    }

    /* Gère les boutons, utilisé dans une closure dans new */
    async fn handle_interaction(&mut self, ctx: &SerenityContext, interaction: &mut ComponentInteraction) -> Result<(), ErrType> {
        if interaction.data.custom_id.starts_with("mm") {
            let id = interaction.data.custom_id.split("-").next()
                .ok_or(ErrType::InteractionIDError(interaction.data.custom_id.clone(), interaction.message.id.get()))?.to_string();
            let next: i32 = if interaction.data.custom_id.split("-").last()
                .ok_or(ErrType::InteractionIDError(interaction.data.custom_id.clone(), interaction.message.id.get()))? == "n" {1} else {-1};
            if let Some(&position) = self.mmpositions.get(&id) {
                let new_pos: usize = ((position as i32) + next) as usize;
                self.mmpositions.insert(id.clone(), new_pos);
                interaction.create_response(ctx, CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(self.multimessages.get(&id).unwrap()[new_pos].clone())
                        .button(CreateButton::new(id.clone() + "-p").label("Précédent")
                            .disabled(new_pos == 0)
                            .style(ButtonStyle::Secondary))
                        .button(CreateButton::new(id.clone() + "-n").label("Suivant")
                            .disabled(new_pos == self.multimessages.get(&id).unwrap().len() - 1)
                            .style(ButtonStyle::Secondary)))
                    ).await?;
            } else {
                /* Multimessage absent: bot reboot? */
                interaction.create_response(ctx, CreateInteractionResponse::Acknowledge).await?;
                interaction.message.edit(ctx, EditMessage::new()
                    .button(CreateButton::new(id.clone() + "-p")
                        .label("Précédent")
                        .disabled(true)
                        .style(ButtonStyle::Secondary))
                    .button(CreateButton::new(id.clone() + "-n")
                        .label("Suivant")
                        .disabled(true)
                        .style(ButtonStyle::Secondary)
                    )).await?;
            }
        } else {
            if let Err(e) = T::buttons(ctx, interaction, self).await {
                match e {
                    ErrType::ObjectNotFound(obj) => {
                        eprintln!("Objet {obj} non trouvé associé au bouton {}. Suppression du message.", interaction.data.custom_id);
                        interaction.message.delete(ctx).await?;
                    },
                    ErrType::InteractionIDError(_, _) => eprintln!("{e}"), /* Tant pis, on va pas faire crash le bot pour un bouton mal formé. */
                    _ => return Err(e)
                }

            } else {
                self.update_affichans(ctx).await?;
            }
        }
        Ok(())
    }

    /// Sauvegarde les écrits dont les identifiants sont donnés.
    ///
    /// Chaque appel à cette fonction crée une nouvelle entrée dans l’historique qui sera
    /// restaurée à chaque appel à [`Bot::annuler`]. Si l’historique contient plus de 5 éléments,
    /// le plus ancien est supprimé.
    ///
    /// Cette fonction règle le drapeau `Bot.update_affichans`
    /// à `true` étant donné que cette fonction doit être systématiquement appelée avant chaque
    /// modification. Cela permet d’éviter de répéter ces deux associations d’actions qui vont
    /// ensemble.
    pub fn archive(&mut self, ids: Vec<u64>){
        if !ids.is_empty() {
            if self.history.len() >= 5 {
                self.history.pop_back();
            }
            self.history.push_front(ids.into_iter().map(
                | id | {
                    (id.clone(), self.database.get(&id).and_then(|o| {Some(o.clone())}))
                }
            ).collect());
        }
        self.update_affichans = true; // Parce que si on archive, c’est qu’on modifie un truc.

    }

    /// Annule la dernière modification, renvie `false` si l’historique est vide.
    ///
    /// L’historique ayant une profondeur maximum de 5, il n’est pas possible d’appeler plus de
    /// cinq fois d’affilée cette méthode.
    pub fn annuler(&mut self) -> bool {
        if let Some(edit) = self.history.pop_front() {
            for (id, ecrit) in edit {
                if let Some(ecrit) = ecrit {
                    self.database.insert(id, ecrit);
                    self.database.get_mut(&id).unwrap().set_modified(true);
                } else {
                    self.database.remove(&id);
                }
            }
            self.update_affichans = true;
            true
        } else {
            false
        }
    }

    /// Sauvegarde la base de données dans son fichier de sauvegarde, au format YAML.
    pub fn save(&self) -> Result<(), ErrType> {
        let mut objects_out = yaml::Array::new();
        for object_id in self.database.keys() {
            objects_out.push(self.database.get(object_id).unwrap().serialize());
        }
        let mut yaml_out = yaml::Hash::new();
        yaml_out.insert(Yaml::String("entries".into()), Yaml::Array(objects_out));
        yaml_out.insert(Yaml::String("last_rss_update".into()), Yaml::Integer(self.last_rss_update.timestamp()));
        let mut out_str = String::new();
        YamlEmitter::new(&mut out_str).dump(&Yaml::Hash(yaml_out))?;
        fs::write(&self.data_file, &out_str)?;
        Ok(())
    }

    /// Recherche un objet d’après son nom.
    ///
    /// La recherche décompose les mots de la chaîne donnée, puis ceux de chaque titre. Si le titre
    /// contient chaque mot du critère, l’écrit est considéré comme répondant au critère demandé.
    /// Un mot du critère est considéré contenu dans le titre lorsqu’il est contenu dans un mot du
    /// titre (et non égal à un mot du titre).
    ///
    /// Exemple : Pour le titre « La Fondation SCP », les critères « fonda »,
    /// « scp » et « fonda scp » seront valides. Par contre, le critère
    /// « fondations » rejettera ce titre.
    pub fn search(&self, s: &str) -> Vec<&u64> {
        let mut results = Vec::new();
        for object_id in self.database.keys() {
            let mut ok = true;
            for mot_s in s.split(" ") {
                let mut found = false;
                for mot in self.database.get(object_id).unwrap().get_name().split(" ") {
                    found = found || basicize(mot).contains(&basicize(mot_s));
                }
                if !found {
                    ok = false;
                    break;
                }
            }
            if ok {
                results.push(object_id);
            }

        }
        results
    }

    /// Envoie les embeds donnés en paramètre au sein d’un seul message à plusieurs pages.
    pub async fn send_embed(&mut self, ctx: &Context<'_, DataType<T>, ErrType>, embeds: Vec<CreateEmbed>) -> Result<(), ErrType> {
        let id = "mm".to_string() + SystemTime::now().elapsed()?.as_millis().to_string().as_str();
        if embeds.len() > 1 {
            self.multimessages.insert(id.clone(), embeds);
            self.mmpositions.insert(id.clone(), 0);
            ctx.send(CreateReply::default()
                .embed(self.multimessages.get(&id).unwrap().first().unwrap().clone())
                .components(vec![CreateActionRow::Buttons(vec![
                    CreateButton::new(id.clone() + "-p")
                        .label("Précédent")
                        .disabled(true)
                        .style(ButtonStyle::Secondary),
                    CreateButton::new(id.clone() + "-n")
                        .label("Suivant")
                        .style(ButtonStyle::Secondary)
                ])])).await?;
        } else {
            ctx.send(CreateReply::default().embed(embeds.first()
                .ok_or(ErrType::EmptyContainer("send_embed appelé avec aucun embed.".to_string()))?.clone())).await?;
        }
        Ok(())
    }

    /// Appelle [`affichan::Affichan::update`] pour tous les affichans, et remet le drapeau
    /// « modifié » des objets à `false` (voir [`object::Object::set_modified`]).
    pub async fn update_affichans(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        for affichan in &mut self.affichans {
            affichan.update(&mut self.database, ctx).await?;
        }
        for (_, ecrit) in &mut self.database {
            ecrit.set_modified(false);
        }
        Ok(())
    }

    /* Fournit l’ID du message supprimé aux salons d’affichage pour éventuellement republier
       le message supprimé si c’était un message d’affichage. */
    async fn check_deletions(&self, ctx: &SerenityContext, message_id: &MessageId) -> Result<(), ErrType> {
        for affichan in &self.affichans {
            affichan.check_message_deletion(self, ctx, message_id).await?;
        }
        Ok(())
    }

    /// Copie un template d’embed en y ajoutant le numéro et le contenu des pages.
    #[deprecated(since = "1.1.0", note = "Déplacé à fondabots_lib::tools::get_multimessages")]
    pub fn get_multimessages(pages: Vec<String>, template: CreateEmbed) -> Vec<CreateEmbed> {
        tools::get_multimessages(pages, template)
    }
}
