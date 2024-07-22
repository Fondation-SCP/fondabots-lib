use chrono::{DateTime, NaiveDate, Utc};
use poise::{Command, Context, CreateReply, serenity_prelude as serenity};
use serenity::all::{ChannelId, CreateEmbed, CreateEmbedFooter, GuildChannel, Timestamp, User, UserId};
use serenity::all::Context as SerenityContext;

use crate::{Bot, DataType, ErrType, Object};

pub trait Preloaded<T> {
    fn load(&self, ctx: &SerenityContext) -> impl std::future::Future<Output = Result<T, ErrType>> + Send;
    fn get(&self) -> Result<&T, ErrType>;
    fn get_mut(&mut self) -> Result<&mut T, ErrType>;
}

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

pub fn basicize(s: &str) -> String {
    s.to_lowercase()
        .replace("é", "e")
        .replace("ê", "e")
        .replace("à", "a")
        .replace("ï", "i")
        .replace("ç", "c")
        .replace("ë", "e")
        .replace("ô", "o")
        .replace("è", "e")
        .replace("î", "i")
        .replace("œ", "oe")
        .replace("æ", "ae")
        .replace("û", "u")
        .replace("ä", "a")
        .replace("ö", "o")
        .replace("â", "a")
        .replace("’", "'").trim().to_string()
}

pub async fn get_object<T: Object>(ctx: &Context<'_, DataType<T>, ErrType>, bot: &Bot<T>, c: &String) -> Result<Option<u64>, ErrType> {
    if let Ok(id) = c.parse() {
        if bot.database.contains_key(&id) {
            Ok(Some(id))
        } else {
            ctx.send(CreateReply::default()
                .content("Aucun écrit n’existe avec cet identifiant.")).await?;
            Ok(None)
        }
    } else {
        let res = bot.search(c);
        if res.len() > 1 {
            ctx.send(CreateReply::default()
                .content("Le nom donné référence plus d’un écrit. Merci d’affiner le critère ou de rechercher par ID."))
                .await?;
            Ok(None)
        } else if res.len() == 0 {
            ctx.send(CreateReply::default()
                .content("Aucun écrit trouvé.")).await?;
            Ok(None)
        } else {
            Ok(Some(res[0].clone()))
        }
    }
}

pub fn parse_date(date: String) -> Option<Timestamp> {
    Some(Timestamp::from(DateTime::<Utc>::from_naive_utc_and_offset(
        NaiveDate::parse_from_str(
            date.as_str(), "%d/%m/%Y").ok().and_then(|d| {
            d.and_hms_opt(0, 0, 0)
        }).unwrap(), Utc)))
}

pub fn alias<T: Object>(name: &str, mut com: Command<DataType<T>, ErrType>) -> Command<DataType<T>, ErrType> {
    com.name = name.to_string();
    com
}

pub fn get_multimessages(pages: Vec<String>, template: CreateEmbed) -> Vec<CreateEmbed> {
    let mut embeds = Vec::new();
    let mut counter = 1;
    let total = pages.len().to_string();
    for page in &pages {
        embeds.push(template.clone()
            .footer(CreateEmbedFooter::new(format!("Page {counter} / {total}")))
            .description(page));
        counter += 1;
    }
    embeds
}

#[allow(deprecated)] /* TODO allow à supprimer en 2.0.0 */
fn _sort_merge<'a, T: Object>(mut a: Vec<(&'a u64, &'a T)>, mut b: Vec<(&'a u64, &'a T)>) -> Vec<(&'a u64, &'a T)> {
    let mut res = Vec::new();
    while !(a.is_empty() && b.is_empty()) {
        if let (Some(ecrit_a), Some(ecrit_b)) = (a.last(), b.last()) {
            res.push(if ecrit_a.1.get_date() < ecrit_b.1.get_date() { &mut a } else { &mut b }.pop().unwrap());
        } else {
            while !if a.is_empty() { &b } else { &a }.is_empty() {
                res.push(if a.is_empty() { &mut b } else { &mut a }.pop().unwrap());
            }
        }
    }
    res.reverse();
    res
}

pub(crate) fn sort_by_date<'a, T: Object>(v: Vec<(&'a u64, &'a T)>) -> Vec<(&'a u64, &'a T)> {
    if v.len() < 1 {
        v
    } else {
        _sort_merge(v[..v.len() / 2].to_vec(), v[v.len() / 2..].to_vec())
    }
}