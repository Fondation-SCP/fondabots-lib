//! Ce module contient de nombreuses fonctions utilitaires utilisées par la bibliothèque ou ajoutées
//! à la suite de besoins communs dans plusieurs utilisation pratiques. Ces differents objets n’ont
//! aucun lien entre eux pour la plupart, sauf spécification contraire.

use crate::{Bot, DataType, ErrType, Object};
use chrono::{DateTime, NaiveDate, Utc};
use poise::futures_util::FutureExt;
use poise::{serenity_prelude as serenity, BoxFuture, Command, Context, CreateReply};
use serenity::all::{ChannelId, CreateEmbed, CreateEmbedFooter, GuildChannel, RoleId, Timestamp, User, UserId};
use serenity::all::{Context as SerenityContext, GetMessages, Message, MessageId};
use std::future::Future;
use unicode_normalization::UnicodeNormalization;

/// Trait utilisé pour des objets de l’API Discord nécessitant un chargement après leur définition.
/// Il permet la récupération d’un tel objet de manière sécurisée afin d’éviter l’utilisation de
/// ce dernier avant son chargement.
///
/// Attention : le statut chargé ou non de l’objet est définitif ; l’appel à la fonction
/// [`Preloaded::load`] rentourne directement l’objet en question. Il est possible de l’utiliser
/// directement, ou de l’encapsuler à nouveau dans un objet [`Preloaded`] prenant en compte son
/// chargement effectif (méthode recommandée).
///
/// Les structures de la bibliothèque implémentant ce trait sont :
/// * [`PreloadedChannel`]
/// * [`PreloadedUser`]
pub trait Preloaded<T> {
    /// Fonction asynchrone qui charge l’objet et renvoie l’objet dans un [`Result`] pour prendre
    /// en compte l’éventuel échec du chargement. Ayant pour but de charger un objet Discord, elle
    /// prend [`serenity::Context`] en paramètre.
    fn load(&self, ctx: &SerenityContext) -> impl Future<Output = Result<T, ErrType>> + Send;

    /// Fournit une référence vers l’objet s’il est chargé, ou une [`ErrType::UnloadedItem`] sinon.
    fn get(&self) -> Result<&T, ErrType>;

    /// Fournit une référence mutable vers l’objet s’il est chargé, ou une [`ErrType::UnloadedItem`]
    /// sinon.
    fn get_mut(&mut self) -> Result<&mut T, ErrType>;
}

/// Représente un salon Discord préchargé (voir [`Preloaded`]) par son identifiant.
pub enum PreloadedChannel {
    Loaded(GuildChannel),
    Unloaded(ChannelId)
}

impl Preloaded<GuildChannel> for PreloadedChannel {
    async fn load(&self, ctx: &SerenityContext) -> Result<GuildChannel, ErrType> {
        Ok(match self {
            Self::Loaded(c) => c.clone(),
            Self::Unloaded(id) => id.to_channel(ctx).await?.guild().ok_or(ErrType::NoneError)?
        })
    }

    fn get(&self) -> Result<&GuildChannel, ErrType> {
        match self {
            Self::Loaded(c) => Ok(c),
            Self::Unloaded(id) => Err(ErrType::UnloadedItem(id.get()))
        }
    }

    fn get_mut(&mut self) -> Result<&mut GuildChannel, ErrType> {
        match self {
            Self::Loaded(c) => Ok(c),
            Self::Unloaded(id) => Err(ErrType::UnloadedItem(id.get()))
        }
    }
}

/// Représente un utilisateur Discord préchargé par son identifiant (voir [`Preloaded`]).
pub enum PreloadedUser {
    Loaded(User),
    Unloaded(UserId)
}

impl Preloaded<User> for PreloadedUser {
    async fn load(&self, ctx: &SerenityContext) -> Result<User, ErrType> {
        Ok(match self {
            Self::Loaded(c) => c.clone(),
            Self::Unloaded(id) => id.to_user(ctx).await?
        })
    }

    fn get(&self) -> Result<&User, ErrType> {
        match self {
            Self::Loaded(c) => Ok(c),
            Self::Unloaded(id) => Err(ErrType::UnloadedItem(id.get()))
        }
    }

    fn get_mut(&mut self) -> Result<&mut User, ErrType> {
        match self {
            Self::Loaded(c) => Ok(c),
            Self::Unloaded(id) => Err(ErrType::UnloadedItem(id.get()))
        }
    }
}

/// Macro permettant de rendre moins verbeux le fait d’ignorer les erreurs dans une boucle.
/// En cas d’erreur de l’action donnée, l’erreur sera affichée dans le log d’erreur puis
/// la boucle passera au prochain élément.
#[macro_export]
macro_rules! try_loop {
    ($e:expr, $m:literal) => (match $e {
        Ok(val) => val,
        Err(err) => {
            eprintln!($m);
            eprintln!("{err}");
            continue;
        }
    })
}

/// Simplifie une chaîne de caractères en la mettant en minuscules, remplaçant certains caractères
/// en caractères équivalents plus communs, et en supprimant les diacritiques et caractères non-ascii.
pub fn basicize(s: &str) -> String {
    s.to_lowercase().replace("`", "'").replace(" ", " ").nfd()
        .filter(|c| c.is_ascii() || c.is_alphanumeric())
        .collect()
}

/// Fonction auxiliaire pour toutes les commandes prenant un objet en argument. Celle-ci va chercher
/// l’objet en question affiche une erreur sur Discord si aucun ou plusieurs objets ont été trouvés
/// correspondant au critère de recherche, en plus de renvoyer [`None`].
///
/// Si le critère de recherche est un nombre, il sera interprété comme l’identifiant de l’objet
/// recherché et la recherche par nom n’aura pas lieu.
pub async fn get_object<T: Object>(ctx: &Context<'_, DataType<T>, ErrType>, bot: &Bot<T>, c: &String) -> Result<Option<u64>, ErrType> {
    if let Ok(id) = c.parse() {
        if bot.database.contains_key(&id) {
            Ok(Some(id))
        } else {
            ctx.send(CreateReply::default()
                .content("Aucun objet n’existe avec cet identifiant.")).await?;
            Ok(None)
        }
    } else {
        let res = bot.search(c);
        if res.len() > 1 {
            ctx.send(CreateReply::default()
                .content("Le nom donné référence plus d’un objet. Merci d’affiner le critère ou de rechercher par ID."))
                .await?;
            Ok(None)
        } else if res.len() == 0 {
            ctx.send(CreateReply::default()
                .content("Aucun objet trouvé.")).await?;
            Ok(None)
        } else {
            Ok(Some(res[0].clone()))
        }
    }
}

/// Lit un [`Timestamp`] au format `%d/%m/%Y` depuis une chaîne de caractères. Renvoie [`None`]
/// si le format de la chaîne de caractères est incorrect.
pub fn parse_date(date: String) -> Option<Timestamp> {
    Some(Timestamp::from(DateTime::<Utc>::from_naive_utc_and_offset(
        NaiveDate::parse_from_str(
            date.as_str(), "%d/%m/%Y").ok().and_then(|d| {
            d.and_hms_opt(0, 0, 0)
        }).unwrap(), Utc)))
}

/// Fonction auxiliaire pour la création d’une commande alias d’une autre commande. Pour l’utiliser,
/// il suffit d’insérer `alias("com_alias", commande_originale())` dans la fonction de déclaration
/// des commandes. La commande d’alias aura automatiquement les mêmes propriétés que la commande
/// originale (description, paramètres), à l’exception de son nom.
pub fn alias<T: Object>(name: &str, mut com: Command<DataType<T>, ErrType>) -> Command<DataType<T>, ErrType> {
    com.name = name.to_string();
    com
}

/// Transforme une liste de chaînes de caractères en une liste d’embeds Discord d’après un
/// template (CreateEmbed déjà pré-rempli, qui sera copié).
///
/// Sont affectés les champs `footer` ([`CreateEmbed::footer`]) pour la liste des pages et
/// `description` ([`CreateEmbed::description`]) pour le contenu des embeds. Attention : il est
/// attendu que chaque chaîne de caractères dans la liste fournie permette de respecter la limite
/// de 2 000 caractères des embeds, sans quoi il y aura une erreur à l’envoi ; cette
/// fonction ne le vérifie pas.
pub fn get_multimessages(pages: Vec<String>, template: CreateEmbed) -> Vec<CreateEmbed> {
    let mut counter = 0;
    let total = pages.len().to_string();
    pages.into_iter().map(|page| {
        counter += 1;
        template.clone()
            .footer(CreateEmbedFooter::new(format!("Page {counter} / {total}")))
            .description(page)
    }).collect()
}

/* Fonction de fusion du tri fusion */
fn _sort_merge<'a, T: Object>(mut a: Vec<(&'a u64, &'a T)>, mut b: Vec<(&'a u64, &'a T)>) -> Vec<(&'a u64, &'a T)> {
    let mut res = Vec::new();
    while !(a.is_empty() && b.is_empty()) {
        if let (Some((_, ecrit_a)), Some((_, ecrit_b))) = (a.last(), b.last()) {
            res.push(if ecrit_a.get_date() > ecrit_b.get_date() { &mut a } else { &mut b }.pop().unwrap());
        } else {
            while !if a.is_empty() { &b } else { &a }.is_empty() {
                res.push(if a.is_empty() { &mut b } else { &mut a }.pop().unwrap());
            }
        }
    }
    res
}

/// Tri un vecteur d’objets (avec leurs identifiants) par date, du plus récent au plus ancien.
pub fn sort_by_date<'a, T: Object>(v: Vec<(&'a u64, &'a T)>) -> Vec<(&'a u64, &'a T)> {
    if v.len() <= 1 {
        v
    } else {
        _sort_merge(sort_by_date(v[..v.len() / 2].to_vec()), sort_by_date(v[v.len() / 2..].to_vec()))
    }
}

/// Crée une liste de pages faisant la liste des objets donnés en paramètre en utilisant la fonction
/// fournie pour définir leur représentation en chaîne de caractères dans la liste. Le paramètre
/// `char_limit` définit la taille maximale de chaque chaîne de caractère de la liste renvoyée.
pub fn create_paged_list<T, F: FnMut(&T) -> String>(mut objects: Vec<T>, mut string_func: F, char_limit: usize) -> Vec<String> {
    match objects.pop() {
        Some(obj) => {
            let obj_str = string_func(&obj);
            let mut rec = create_paged_list(objects, string_func, char_limit);
            if rec.is_empty() {
                vec![obj_str]
            } else {
                if rec.last().unwrap().len() + obj_str.len() > char_limit {
                    rec.push(obj_str);
                } else {
                    let last_str = rec.pop().unwrap();
                    rec.push(last_str + obj_str.as_str());
                }
                rec
            }
        },
        None => vec![]
    }
}

/// Récupère tous les messages d’un salon Discord depuis le messae indiqué (ou sa création si [`None`]). 
/// Attention : cette commande
/// peut prendre un certain temps à s’exécuter. Fonction récursive asynchrone renvoyant BoxFuture.
pub fn get_channel_messages<'a>(chan: &'a GuildChannel, ctx: &'a SerenityContext, before: Option<&'a MessageId>) -> BoxFuture<'a, Result<Vec<Message>, ErrType>> {
    async move {
        let mut get_messages = GetMessages::new().limit(100);
        if let Some(before) = before {
            get_messages = get_messages.before(before);
        }
        let current_messages = chan.messages(ctx, get_messages).await?;
        Ok(vec![
            if let Some(last_message) = current_messages.last() {
                get_channel_messages(chan, ctx, Some(&last_message.id)).await?
            } else {
                vec![]
            },
            current_messages
        ].concat())
    }.boxed()
}

pub async fn check_for_role<T: Object>(ctx: &Context<'_, DataType<T>, ErrType>, role: RoleId) -> Result<bool, ErrType> {
    let member = ctx.author_member().await;
    if let Some(member) = member {
        if !member.roles.contains(&role) {
            ctx.reply("Vous n'avez pas l'autorisation d'utiliser cette commande.").await?;
            Ok(false)
        } else {
            Ok(true)
        }
    } else {
        ctx.reply("Échec de la vérification de l'autorisation d'utiliser la commande. Réessayez plus tard.").await?;
        Ok(false)
    }
}

pub fn user_desc(user: &User) -> String {
    format!("{} (id: {})", user.display_name(), user.id)
}