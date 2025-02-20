//! Module contenant la structure [`Affichan`].

use errors::Error;
use poise::futures_util::{StreamExt, TryStreamExt};
use poise::serenity_prelude as serenity;
use serenity::all::Message;
use serenity::all::{ChannelId, MessageId, UserId};
use serenity::all::{Context as SerenityContext, Context};
use serenity::futures::future::{join_all, try_join_all};
use std::collections::HashMap;
use std::mem::take;
use std::sync::Arc;
use tools::PreloadedChannel;
use yaml_rust2::{yaml, Yaml};

use crate::tools::Preloaded;
use crate::Bot;
use crate::ErrType;
use crate::{errors, tools, Object};

/// Un salon d’affichage du bot.
///
/// Ces salons d’affichage ont pour but d’afficher un certain nombre de messages d’objets correspondant
/// au test donné. Ces messages peuvent lister une certaine catégorie définie d’objets, et chaque
/// message peut avoir un certain nombre de boutons ayant des actions définies par l’utilisateur
/// de la librairie (implémentation de [`Object`]).
///
/// Les différents Affichans sont crées à la création du bot (voir [`Bot::new`]) et sont ensuite
/// stockés dans un champ du [`Bot`] et ne sont pas accessibles directement. Il est cependant possible
/// de forcer la mise à jour des affichans par l’appel à [`Bot::update_affichans`] qui appelle
/// la fonction [`Affichan::update`] pour chaque Affichan donné au chargement du bot.
pub struct Affichan<T: Object> {
    /// Le salon Discord du salon d’affichage.
    chan: PreloadedChannel,
    /// La liste des messages Discord contenus dans le salon, indexés par identifiant d’objet selon
    /// la [`HashMap`] contenue dans [`Bot`].
    messages: HashMap<u64, Message>,
    /// Fonction qui doit renvoyer `true` si l’objet doit appartenir au salon d’affichage.
    /// Attention : l’objet est fourni en tant que [`Option`] étant donné que l’existence
    /// de l’objet n’est pas assurée lors de l’utilisation de ces tests. Il convient à l’utilisateur
    /// de cette librairie de prendre en compte le cas où celle-ci serait [`None`].
    test: Box<dyn Fn(Option<&T>) -> bool + Sync + Send + 'static>
}

impl<T: Object> Affichan<T> {
    /// Créé un nouvel Affichan vide avec la fonction de test fournie.
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

    /* Charge le salon préchargé en tant qu’objet de l’API Discord */
    async fn _load(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        self.chan = PreloadedChannel::Loaded(self.chan.load(ctx).await?);
        Ok(())
    }

    /// Sauvegarde les Affichans dans un objet YAML.
    ///
    /// Cette fonction est appelée automatiquement dans [`Bot::save`] pour tous les Affichans du bot.
    pub fn save(&self) -> Yaml {
        Yaml::Array(self.messages.iter().map(|(&object_id, message)| {
            let mut out = yaml::Hash::new();
            out.insert(Yaml::String("id".to_string()), Yaml::Integer(object_id as i64));
            out.insert(Yaml::String("message_id".to_string()), Yaml::Integer(message.id.get() as i64));
            Yaml::Hash(out)
        }).collect())
    }

/* Charge une sauvegarde d’Affichan. Fonction utilisée dans init. */
    async fn _load_from_save(&self, saved_data: &Yaml, ctx: &SerenityContext) -> Result<HashMap<u64, Message>, ErrType> {
        println!("Chargement à partir d'une sauvegarde d'affichan…");
        /* Clonage gênant (clone toutes les données yaml de l'affichan) mais nécessaire pour pouvoir
           refiler les données aux threads.
         */
        let saved_data = saved_data.as_vec()
            .ok_or(ErrType::YamlParseError("Erreur de yaml dans les affichans: pas un tableau.".to_string()))?
            .iter().filter_map(|yaml_message| match yaml_message.as_hash() {
                Some(_) => Some((yaml_message["id"].as_i64(), yaml_message["message_id"].as_i64())),
                None => {eprintln!("Attention : l'une des entrées d'une sauvegarde d'affichan n'est pas un Hash."); None}
            }).collect::<Vec<_>>();

        let ctx = Arc::new(ctx.clone());
        let chan = Arc::new(self.chan.get().unwrap().clone());

        let futures = saved_data.into_iter()
            .map(move |(object_id, message_id)| {
            let ctx = ctx.clone();
            let chan = chan.clone();
            async move {
                if object_id.is_none() || message_id.is_none() {
                    Err(ErrType::YamlParseError("Erreur de yaml dans un affichan: un identifiant n’est pas un entier.".into()))
                } else {
                    let message_id = message_id.unwrap() as u64;
                    println!("Récupération du message {message_id}…");
                    match chan.message(ctx, MessageId::new(message_id)).await {
                        Ok(message) => Ok(Some((object_id.unwrap() as u64, message))),
                        Err(_) => {eprintln!("Message {message_id} non trouvé sur Discord. Tant pis."); Ok(None)}
                    }
                }
            }
        });

        Ok(tokio_stream::iter(futures).buffer_unordered(4).try_collect::<Vec<_>>().await?
            .into_iter().filter_map(|x| x).collect())
    }

    /* Retrouve les objets de l’Affichan d’après les messages déjà présents dans le salon Discord. Fonction utilisée dans init. */
    async fn _load_from_messages(&self, database: &HashMap<u64, T>, self_id: &UserId, messages: Vec<Message>, ctx: &Context) -> Result<HashMap<u64, Message>, Error> {
        println!("Chargement à partir des messages…");
        let self_messages = &self.messages;

        Ok(join_all(messages.into_iter().filter(|message|
            message.author.id.get() == self_id.get()
                && !message.embeds.is_empty()
        )
            .filter_map(|message| message.embeds.get(0).unwrap()
                .footer.as_ref()
                .and_then(|footer| footer.text.parse().ok())
                .and_then(|footer_text| Some((message, footer_text))))
            .map(|(message, footer_id)| async move {
                if let Some(object) = database.get(&footer_id) {
                    if !self_messages.contains_key(&object.get_id()) {
                        Some((object.get_id(), message.clone()))
                    } else {
                        eprintln!("Message {} en trop: suppression.", message.id);
                        let ctx = ctx.clone();
                        tokio::spawn(async move {message.delete(ctx).await});
                        None
                    }
                } else {
                    eprintln!("Message {} sans objet associé: message supprimé.", message.id);
                    let ctx = ctx.clone();
                    tokio::spawn(async move {message.delete(ctx).await});
                    None
                }
            })).await
            .into_iter().filter_map(|option| option).collect())
    }

    /// Initiatise l’Affichan en chargeant ses données à partir d’une sauvegarde ou des messages
    /// déjà présents dans le salon Discord si celle-ci n’existe pas. Cet appel n’est nécessaire
    /// qu’une seule fois et est fait automatiquement dans le setup (défini dans [`Bot::new`]) pour
    /// tous les Affichan déclarés.
    ///
    /// Appelle [`Affichan::update`] après le chargement des messages.
    pub async fn init(&mut self, database: &HashMap<u64, T>, self_id: &UserId, saved_data: Option<&Yaml>, ctx: &SerenityContext) -> Result<(), ErrType> {
        self._load(ctx).await?;

        self.messages = match saved_data {
            Some(saved_data) => self._load_from_save(saved_data, ctx).await,
            None => self._load_from_messages(database, self_id, tools::get_channel_messages(self.chan.get()?, ctx, None).await?, ctx).await
        }?;

        self.update(database, ctx).await
    }


    /// Met à jour le salon d’affichage en modifiant les objets présents s’ils ont été modifiés,
    /// supprimant les objets qui n’y ont plus leur place et ajoutant les objets qui devraient
    /// y être.
    ///
    /// Utilisée par [`Bot::update_affichans`] qui appelle cette fonction pour tous les Affichans.
    pub async fn update(&mut self, database: &HashMap<u64, T>, ctx: &SerenityContext) -> Result<(), ErrType> {

        /* Met à jour les objets déjà présents dans la base de données */
        let edit_fails = self._edit_messages_if_modified(database, ctx).await;

        let mut deleted_elements = Vec::new();

        self.messages.retain(|object_id, message| { 
                let keep = /* on garde si */
                    database.contains_key(object_id) && /* dans la bdd */
                    (self.test)(database.get(object_id)) && /* true au test */
                    !edit_fails.contains(object_id);
                if !keep {
                    deleted_elements.push(take(message));
                }
                keep
            }
        );

        let ctx_arc_deletions = Arc::new(ctx.clone());
        let ctx_arc_messages = ctx_arc_deletions.clone();

        tokio::spawn(tokio_stream::iter(deleted_elements.into_iter().map( move |message| {
            let ctx = ctx_arc_deletions.clone(); /* Arc clone */
            async move {message.delete(ctx).await}
        })).buffer_unordered(4).collect::<Vec<_>>());

        let self_chan = &self.chan;
        let self_test = &self.test;

        let futures_messages = tools::sort_by_date(self._get_new_valid_objects_from_db(database, self_test))
            .into_iter().rev()
            .map(|(&object_id, object)| (object_id, object.get_message()))
            .collect::<Vec<_>>().into_iter() /* Nettoyage de l'ancien itérateur pour y retirer les références */
            .map(|(object_id, create_message)| { /* Comme ça les blocs async ne capturent aucune référence */
            let ctx = ctx_arc_messages.clone();
            let chan = self_chan.get().unwrap().clone();
            async move {
                Ok::<_, ErrType>(
                    (object_id, chan.send_message(ctx, create_message).await?)
                )
            }
        });

        self.messages.extend(tokio_stream::iter(futures_messages).buffered(4).try_collect::<Vec<_>>().await?);
        Ok(())
    }

    /* Renvoie tous les objets de la bdd qui ne sont pas déjà présents dans l’Affichan et
     * qui passent la fonction test. */
    fn _get_new_valid_objects_from_db<'a>(&self, database: &'a HashMap<u64, T>, test: &Box<dyn Fn(Option<&T>) -> bool + Sync + Send + 'static>) -> Vec<(&'a u64, &'a T)> {
        database.iter()
            .filter(|(id, obj)|
                (*test)(Some(obj)) && !self.messages.contains_key(id)
            ).collect()
    }

    /* Modifie tous les écrits valides (présents dans la BDD & respectant le test) et renvoie ceux
     * dont la modification a échoué (message inexistant le plus souvent).
     * Fonction utilisée dans update.
     */
    async fn _edit_messages_if_modified(&mut self, database: &HashMap<u64, T>, ctx: &Context) -> Vec<u64> {
        join_all(self.messages.iter_mut().filter(|(object_id, _)|
             (self.test)(database.get(object_id)) && database.get(object_id).is_some_and(|object| object.is_modified())
        ).map(|(object_id, message)| async {
            match message.edit(ctx, database.get(object_id).unwrap().get_message_edit()).await {
                Err(_) => Some(*object_id),
                Ok(_) => None
            }
        })).await.into_iter().filter_map(|x| x).collect()
    }

    /// Appelle [`Affichan::refresh`] et supprime en plus tous les objets de l’affichan. Les objets valides
    /// seront réinsérés au prochain appel à la fonction [`Affichan::update`].
    pub async fn purge(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        self.refresh(ctx).await?;
        self.messages.clear();
        Ok(())
    }

    /// Supprime tous les messages de l’affichan sans pour autant supprimer tous les objets.
    /// La suppression des messages sera détectée par `Bot::check_deletions`, qui appelle
    /// [`Affichan::check_message_deletion`] pour tous les Affichan. Les messages seront donc republiés par
    /// la suite. N’a aucun impact sur la liste des objets de l’affichan, seulement sur les messages.
    pub async fn refresh(&mut self, ctx: &SerenityContext) -> Result<(), ErrType> {
        let ctx_arc = Arc::new(ctx.clone());
        let futures = self.messages.clone() /* Pour le lancer dans des threads */
            .into_iter().map(|(_, message)| {
            let ctx = ctx_arc.clone();
            async move {
                message.delete(ctx).await
            }
        });
        tokio_stream::iter(futures).buffer_unordered(4).try_collect::<Vec<_>>().await?;
        Ok(())
    }

    /// Vérifie si un message supprimé correspond à un message de l’affichan. Si c’est le cas,
    /// republie le message en question.
    pub async fn check_message_deletion(&self, bot: &Bot<T>, ctx: &SerenityContext, message_id: &MessageId) -> Result<(), ErrType> {
        let deleted = self.messages.iter().find(|(_, message)| message.id.get() == message_id.get());
        if let Some((object_id, _)) = deleted {
            let chan = self.chan.get()?;
            let object = bot.database.get(object_id)
                .ok_or(Error::ObjectNotFound(format!("Objet {} référencé dans un message supprimé dans Affichan {} (id: {})", *object_id, chan.name, chan.id)))?;
            chan.send_message(ctx, object.get_message()).await?;
        }

        Ok(())
    }

    /// Supprime un message particulier de l’affichan. Cette suppression sera détectée par
    /// `Bot::check_deletions`, qui appelle [`Affichan::check_message_deletion`] qui republiera le message.
    /// Le principal intérêt de cette méthode est de remettre un message en bas du salon.
    pub async fn up(&self, ctx: &SerenityContext, object_id: &u64) -> Result<(), ErrType> {
        self.messages.get(object_id)
            .ok_or(Error::ObjectNotFound(
                format!("Écrit {object_id} non trouvé dans Affichan {} (id: {})",
                        self.chan.get()?.name, self.chan.get()?.id)))?
            .delete(ctx).await?;
        Ok(())
    }

    /// Supprime un écrit précis de l’affichan et supprime son message. Cet écrit sera à nouveau
    /// rajouté au prochain appel à [`Affichan::update`] s’il existe dans la base de données et correspond
    /// toujours aux critères.
    pub async fn remove(&mut self, ctx: &SerenityContext, object_id: &u64) -> Result<(), ErrType> {
        self.messages.get(object_id).
            ok_or(Error::ObjectNotFound(
            format!("Écrit {object_id} non trouvé dans Affichan {} (id: {})",
                    self.chan.get()?.name, self.chan.get()?.id)))?
            .delete(ctx).await?;
        self.messages.remove(object_id);
        Ok(())
    }

    /// Met à jour tous les messages de l’affichan, que l’objet qu’ils référencent ait été modifié
    /// ou non. S’arrête à la première erreur et la renvoie.
    ///
    /// Cette fonction a un rôle différent de la fonction privée `Affichan::_edit_messages_if_modified` qui
    /// ne modifie que les objet ayant le drapeau `modified` activé, qui passe les erreurs et renvoie
    /// les identifiants des objets dont la modification a échoué.
    pub async fn edit_all_messages(&mut self, bot: &Bot<T>, ctx: &SerenityContext) -> Result<(), ErrType> {
        try_join_all(
            self.messages.iter_mut().filter_map(|(object_id, message)| bot.database.get(object_id)
                .map_or_else(|| None, |object| Some((object, message))))
            .map(|(object, message)| message.edit(ctx, object.get_message_edit()))
        ).await?;
        Ok(())
    }

    /// Vérifie si un objet est contenu dans l’affichan.
    pub fn contains_object(&self, object_id: &u64) -> bool {
        self.messages.contains_key(object_id)
    }

    /// Renvoie l’identifiant du salon Discord, qu’il ait été chargé ou non.
    pub fn get_chan_id(&self) -> u64 {
        match &self.chan {
            PreloadedChannel::Loaded(guild_channel) => &guild_channel.id,
            PreloadedChannel::Unloaded(channel_id) => channel_id
        }.get()
    }

}