use crate::{DataType, ErrType, Object};
use poise::futures_util::FutureExt;
use poise::{BoxFuture, Context};

///! Ce module définit le type utilisé pour [`poise::structs::Command::custom_data`]. Actuellement,
///! il n'est utilisé que pour [`Permission`], mais il pourrait avoir d'autres usages à l'avenir.

/// Définit le type d'une fonction permettant de vérifier qu'un utilisateur a bien le droit
/// d'utiliser une commande.
///
/// Elle prend deux paramètres : une [`Permission`] et le [`Context`] de la commande,
/// et renvoie un bloc async boxé renvoyant un [`Result`] de paramètres booléen et [`ErrType`].
///
/// Note : il faut utiliser [`FutureExt::boxed`] après le bloc async.
pub type CommandChecker<T> = dyn Fn(Context<'_, DataType<T>, ErrType>) -> BoxFuture<'_, Result<bool, ErrType>> + Send + Sync;


/// Ce type est la structure utilisée pour le [`poise::structs::Command::custom_data`].
///
/// Chaque type des champs de [`CommandData`] dérive de [`Default`] et permet donc une configuration facile.
/// Dans le cas où [`poise::structs::Command::custom_data`], la commande de vérification
/// utilisera la valeur par défaut de chaque champ.
///
/// En cas d'utilisation directe de la structure `CommandData {}`, incluez systématiquement
/// les valeurs par défaut `..CommandData::default()` même s'il n'y en a pas d'autres.
/// Une mise à jour ajoutant une CommandData ne sera pas considérée comme potentiellement cassante,
/// mais l'absence des valeurs par défaut pourra rendre votre code non-compilable tant que les
/// nouvelles valeurs n'ont pas été définies.
#[derive(Default)]
pub struct CommandData {
    /// Contient le niveau de [`Permission`] de la commande.
    pub permission: Permission
}

unsafe impl Send for CommandData {}
unsafe impl Sync for CommandData {}
impl CommandData {

    /// Commande de vérification appelant le champ `command_checker` de [`crate::Bot`], permettant
    /// ainsi à l'utilisateur de cette librairie de définir sa propre fonction de vérification.
    pub fn check<T: Object>(ctx: Context<'_, DataType<T>, ErrType>) -> BoxFuture<'_, Result<bool, ErrType>> {
        async move {
            let bot = &mut ctx.data().lock().await;
            (bot.command_checker)(ctx).await
        }.boxed()
    }

    /// Raccourci pour créer un [`CommandData`] ne divergeant que par le niveau de [`Permission`]
    /// de la commande.
    pub fn perms(permission: Permission) -> CommandData {
        CommandData { permission, ..CommandData::default() }
    }
}

/// Définit les différents niveaux de permission des commandes du bot.
#[derive(Clone, Copy)]
pub enum Permission {
    /// La commande ne modifie pas la base de données.
    READ,
    /// La commande modifie légèrement la base de données, dans le cadre d'une utilisation.
    WRITE,
    /// La commande modifie lourdement la base de données, dans le cadre de la gestion de cette dernière.
    MANAGE,
    /// La commande ne bénéficie d'aucun système de permissions (défaut).
    NONE
}

impl Default for Permission {
    fn default() -> Self {
        Permission::NONE
    }
}