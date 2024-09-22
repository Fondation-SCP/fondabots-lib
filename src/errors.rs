//! Définit la structure d’erreur utilisée par la bibliothèque. Contient de nombreuses erreurs
//! spécifiques à fondabots en plus de la retransmission d’erreurs des bibliothèques utilisées.
use std::fmt::{Debug, Display, Formatter};
use std::time::SystemTimeError;

/// Objet d’erreur utilisé par fondabots.
///
/// Il définit plusieurs types d’erreurs courantes et intègre certaines erreurs des bilbiothèques
/// externes.
#[derive(Debug)]
pub enum Error {
    /// Un objet n’a pas été trouvé dans la base de données alors qu’il était attendu d’en trouver un.
    ObjectNotFound(String),
    /// Un conteneur est vide alors qu’il n’aurait pas dû l’être.
    EmptyContainer(String),
    SerenityError(serenity::Error),
    YamlEmitError(yaml_rust2::EmitError),
    IOError(std::io::Error),
    SystemTimeError(SystemTimeError),
    /// Erreur de lecture de Yaml.
    YamlParseError(String),
    /// Identifiant d’interaction invalide en comparaison de ce qui était attendu. Contient
    /// l’identifiant en question et l’ID du message.
    InteractionIDError(String, u64),
    /// Présence d’un [`None`] qui n’aurait pas dû être là.
    NoneError,
    /// Objet Discord préchargé non chargé. Voir [`crate::tools::Preloaded`],
    /// [`crate::tools::PreloadedChannel`]  et [`crate::tools::PreloadedUser`]. Contient
    /// l’identifiant de cet objet.
    UnloadedItem(u64),
    /// Erreur dans l’utilisation d’une commande.
    CommandUseError(String),
    /// Erreur générique, à éviter d’utiliser. Prévue pour les erreurs qui ne devraient pas pouvoir
    /// exister (condition préalable vérifiée en amont mais indication de l’erreur obligatoire
    /// par exemple). En général, l’utilisation de ce type d’erreur est le signe d’un mauvais
    /// code, mais cela peut être utile de passer par là en première instance.
    Generic
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ObjectNotFound(e) => write!(f, "Objet non trouvé : {e}"),
            Error::EmptyContainer(e) => write!(f, "Conteneur vide : {e}"),
            Error::SerenityError(e) => Display::fmt(&e, f),
            Error::YamlEmitError(e) => Display::fmt(&e, f),
            Error::IOError(e) => Display::fmt(&e, f),
            Error::SystemTimeError(e) => Display::fmt(&e, f),
            Error::YamlParseError(e) => write!(f, "Erreur de formatage yaml : {e}"),
            Error::InteractionIDError(id, message) => write!(f, "Erreur de format de l’identifiant {id} sur le message {message}"),
            Error::NoneError => write!(f, "Option None non-attendue."),
            Error::UnloadedItem(id) => write!(f, "Affichan {id} appelé mais non chargé."),
            Error::Generic => write!(f, "Erreur de bot générique."),
            Error::CommandUseError(e) => write!(f, "Erreur d’utilisation de la commande : {e}"),
        }
    }
}



unsafe impl Send for Error {}

unsafe impl Sync for Error  {}

impl std::error::Error for Error {}

impl From<serenity::Error> for Error {
    fn from(e: serenity::Error) -> Self {Error::SerenityError(e)}
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {Error::IOError(e)}
}

impl From<yaml_rust2::EmitError> for Error {
    fn from(e: yaml_rust2::EmitError) -> Self {Error::YamlEmitError(e)}
}

impl From<SystemTimeError> for Error {
    fn from(e: SystemTimeError) -> Self {Error::SystemTimeError(e)}
}