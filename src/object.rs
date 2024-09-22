//! Ce module définit les traits [`Object`] et [`Field`] qui définissent le type d’objet
//! traité par le bot utilisant cette bibliothèque. [`Field`] est conçu pour être une abstraction
//! des nombreuses propriétés que peut contenir un [`Object`], et permet l’utilisation des fonctions
//! auxiliaires définies dans [`crate::generic_commands`] permettant de créer des fonctions interagissant
//! avec ces champs très facilement.
use std::fmt::{Debug, Display};
use std::str::FromStr;

use poise::CreateReply;
use poise::{serenity_prelude as serenity, ChoiceParameter};
use serenity::all::ArgumentConvert;
use serenity::Context as SerenityContext;
use serenity::{ComponentInteraction, CreateActionRow, CreateEmbed, CreateMessage, EditMessage, Timestamp};
use yaml_rust2::Yaml;

use crate::{Bot, DataType, ErrType};

/// Ce trait définit un objet tel qu’utilisé par [`Bot`]. Le bot ne contient qu’une seule
/// base de données ; il n’est donc prévu qu’une seule instanciation de ce trait par bot.
///
/// ## Propriétés
///
/// Un objet implémentant ce trait peut prendre toutes les propriétés qu’il le souhaite, mais
/// certaines de ces fonctions sont prévues pour utiliser des propriétés relativement génériques
/// qui devraient être communes à tous les objets :
/// * Un nom (requis par [`Object::get_name`] et [`Object::set_name`])
/// * Un drapeau de modification pour permettre la mise à jour de l’objet dans les salons
/// d’affichage ([`crate::affichan::Affichan`]) (requis par [`Object::is_modified`] et [`Object::set_modified`])
/// * Un identifiant `u64` (requis par [`Object::get_id`]), utilisé pour le stockage dans la base
/// de données.
/// * Une date (requis par [`Object::get_date`] et [`Object::set_date`]).
///
/// Dans le cas où il serait souhaité de ne pas avoir l’une de ces propriétés, il est toujours
/// possible de faire retourner aux fonctions correspondantes une valeur par défaut. Attention
/// cependant, cela n’est pas testé et est donc susceptible de causer des dysfonctionnements internes
/// à la bibliothèque fondabots.
///
/// ## Caractéristiques
///
/// Les structures [`Object`] doivent également partager un certain nombre de caractéristiques communes :
/// * Pouvoir être converties depuis et vers un format YAML (requis par [`Object::from_yaml`] et
/// [`Object::serialize`])
/// * Être compatible avec l’affichage dans un salon d’affichage :
///   * Par un message individuel contenant un embed : [`Object::get_embed`]
///   * Par la modification avec des boutons personnalisés : [`Object::get_buttons`]
/// * Pouvoir être affiché dans une liste de résultats de recherche (requis par [`Object::get_list_entry`])
///
/// ## Fonctions statiques
///
/// Le trait [`Object`] définit également des fonctions statiques asynchrones qui définissent
/// certains aspects du fonctionnement du bot dépendant fortement de l’implémentation du trait :
/// * [`Object::buttons`] : traitement de l’évènement Discord "appui d’un bouton".
/// * [`Object::maj_rss`] : gestion de la mise à jour de la base de données depuis un flux RSS.
///
/// ## Fonctions prédéfinies
///
/// Les fonctions suivantes sont déjà prédéfinies et utilisées par la bibliothèque fondabots.
/// Ces fonctions créent un message contenant l’embed de [`Object::get_embed`] et
/// les boutons de [`Object::get_buttons`], et le renvoie chacune sous une forme différente.
/// * [`Object::get_message`] : renvoie un [`CreateMessage`].
/// * [`Object::get_message_edit`] : renvoie un [`EditMessage`].
/// * [`Object::get_reply`] : renvoie un [`CreateReply`].
pub trait Object: Send + Sync + 'static + PartialEq + Clone + Debug {
    #[deprecated(since = "1.1.0", note = "Méthode inutile. Sera supprimée en 2.0.0")]
    fn new() -> Self {
        unimplemented!("Méthode supprimée.")
    }
    /// Renvoie l’identifiant de l’objet. Cet identifiant doit être le même que celui utilisé
    /// dans la base de données de [`Bot`] sous peine de causer des comportements imprévisibles.
    fn get_id(&self) -> u64;

    /// Renvoie un nouvel [`Object`] d’après des données au format [`Yaml`]. La structure de ce
    /// format est laissée libre, mais doit être cohérente avec [`Object::serialize`].
    fn from_yaml(data: &Yaml) -> Result<Self, ErrType>;

    /// Convertit un [`Object`] au format [`Yaml`]. La structure de ce
    /// format est laissée libre, mais doit être cohérente avec [`Object::from_yaml`].
    fn serialize(&self) -> Yaml;

    /// Si `true`, indique aux [`crate::affichan::Affichan`] que le message correspondant à l’objet doit être mis à jour.
    /// Sera remis à `false` par un appel à [`Object::set_modified`] une fois la mise à jour appliquée.
    fn is_modified(&self) -> bool;

    /// Change le drapeau de modification récente. Voir [`Object::is_modified`]. Appelé automatiquement
    /// une fois la mise à jour faite dans les [`crate::affichan::Affichan`].
    fn set_modified(&mut self, modified: bool);

    /// Renvoie l’embed correspondant à l’objet.
    ///
    /// <div class="warning">
    /// Pour le bon fonctionnement de l’initialisation des Affichan d’après les messages
    /// qui y sont déjà, l’identifiant de l’objet (voir Object::get_id) doit impérativement
    /// se trouver dans le footer de l’embed.
    /// </div>
    fn get_embed(&self) -> CreateEmbed;

    /// Renvoie les boutons qui apparaissent sous les messages individuels des objets.
    /// Il est possible de n’en inclure aucun en laissant l’action row vide.
    ///
    /// Chaque bouton doit avoir un traitement défini dans [`Object::buttons`].
    ///
    /// <div class="warning">
    /// Les identifiants de boutons commençant par "mm-" sont réservés pour le traitement des
    /// messages à plusieurs pages. Utiliser un tel identifiant ailleurs causera un mauvais traitement
    /// du bouton et des résultats imprévisibles (mais certainement pas ceux voulus, car Object::buttons
    /// ne sera pas appelé).
    /// </div>
    fn get_buttons(&self) -> CreateActionRow;

    /// Renvoie un [`CreateMessage`] créant un message contenant l’embed de [`Object::get_embed`]
    /// et les boutons de [`Object::get_buttons`].
    fn get_message(&self) -> CreateMessage {
        CreateMessage::new().embed(self.get_embed()).components(vec![self.get_buttons()])
    }

    /// Renvoie un [`EditMessage`] remplaçant un message par un autre contenant l’embed de
    /// [`Object::get_embed`] et les boutons de [`Object::get_buttons`].
    fn get_message_edit(&self) -> EditMessage {
        EditMessage::new().embed(self.get_embed()).components(vec![self.get_buttons()])
    }

    /// Renvoie un [`CreateReply`] créant une réponse contenant l’embed de [`Object::get_embed`]
    /// et les boutons de [`Object::get_buttons`].
    fn get_reply(&self) -> CreateReply {
        CreateReply::default().embed(self.get_embed()).components(vec![self.get_buttons()])
    }

    /// Renvoie le nom de l’objet.
    fn get_name(&self) -> &String;

    /// Change le nom de l’objet.
    fn set_name(&mut self, s: String);

    /// Renvoie les quelques lignes de l’entrée de l’objet pour l’affichage dans une liste
    /// de résultats.
    fn get_list_entry(&self) -> String;

    /// Méthode appelée dans la commande par défaut `/up` qui supprime l’objet des [`crate::affichan::Affichan`] pour
    /// republier le message correspondant en tant que message le plus récent des salons.
    ///
    /// Cette méthode permet d’effectuer des actions supplémentaires, comme modifier des propriétés.
    fn up(&mut self);

    /// Fonction traitant les boutons définis dans [`Object::get_buttons`].
    ///
    /// <div class="warning">
    /// Les identifiants de boutons commençant par "mm-" sont réservés pour le traitement des
    /// messages à plusieurs pages. Utiliser un tel identifiant ailleurs causera un mauvais traitement
    /// du bouton et des résultats imprévisibles (mais certainement pas ceux voulus, car Object::buttons
    /// ne sera pas appelé).
    /// </div>
    fn buttons(ctx: &SerenityContext, interaction: &mut ComponentInteraction, bot: &mut Bot<Self>) -> impl std::future::Future<Output = Result<(), ErrType>> + Send;

    /// Fonction traitant les mises à jour de la base de données d’après un flux CSS.
    fn maj_rss(bot: &DataType<Self>) -> impl std::future::Future<Output = Result<(), ErrType>> + Send;

    /// Renvoie la date de l’objet.
    ///
    /// <div class="warning">
    /// Bien que ça ne soit pas requis pour la compilation pour des raisons de compatiblité rétroactive,
    /// tout appel à cette méthode non-réimplémentée aboutira en une panique.
    /// </div>
    fn get_date(&self) -> &Timestamp {
        unimplemented!("Cette méthode devrait être ré-implémentée.")
    }

    /// Modifie la date de l’objet.
    ///
    /// <div class="warning">
    /// Bien que ça ne soit pas requis pour la compilation pour des raisons de compatiblité rétroactive,
    /// tout appel à cette méthode non-réimplémentée aboutira en une panique.
    /// </div>
    fn set_date(&mut self, _t: Timestamp) {
        unimplemented!("Cette méthode devrait être ré-implémentée.")
    }
}

/// Ce trait permet d’utiliser les fonctions auxiliaires génériques de [`crate::generic_commands`] sur des
/// propriétés spécifiques à une implémentation de [`Object`] (comme des énumérations par exemple).
pub trait Field<T: Object>: Eq + ChoiceParameter + Display + Clone + Sync + ArgumentConvert + Send + FromStr {
    /// Vérifie que la propriété du type [`Field`] de l’objet correspond à celle donnée en paramètre.
    ///
    /// Permet également de définir le comportement "par défaut" dans le cas où la propriété donnée
    /// est [`None`]. Est utilisée dans les fonctions auxiliaires génériques de recherche.
    fn comply_with(obj: &T, field: &Option<Self>) -> bool;

    /// Change la propriété du type [`Field`] de l’objet par celle donnée en paramètre.
    fn set_for(obj: &mut T, field: &Self);

    /// Renvoie une chaîne de caractères statiques correspondant au nom du paramètre.
    ///
    /// En général, cela correspond simplement au nom de la structure implémentant [`Field`], ou à
    /// une version plus naturelle de celle-ci (avec espaces et accents par exemple).
    fn field_name() -> &'static str;
}