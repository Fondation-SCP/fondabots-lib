use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub enum Error {
    ObjectNotFound(String),
    EmptyContainer(String),
    LibError(Box<dyn std::error::Error + Send + Sync + 'static>),
    YamlParseError(String),
    InteractionIDError(String, u64),
    NoneError,
    UnloadedItem(u64),
    CommandUseError(String),
    Generic
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ObjectNotFound(e) => write!(f, "Objet non trouvé : {e}"),
            Error::EmptyContainer(e) => write!(f, "Conteneur vide : {e}"),
            Error::LibError(e) => Display::fmt(&e, f),
            Error::YamlParseError(e) => write!(f, "Erreur de formatage yaml : {e}"),
            Error::InteractionIDError(id, message) => write!(f, "Erreur de format de l’identifiant {id} sur le message {message}"),
            Error::NoneError => write!(f, "Option None non-attendue."),
            Error::UnloadedItem(id) => write!(f, "Affichan {id} appelé mais non chargé."),
            Error::Generic => write!(f, "Erreur de bot générique."),
            Error::CommandUseError(e) => write!(f, "Erreur d’utilisation de la commande : {e}"),
        }
    }
}



unsafe impl std::marker::Send for Error {}

unsafe impl Sync for Error  {}

impl<E: std::error::Error + Sync + Send + 'static> From<E> for Error {
    fn from(value: E) -> Self {
        Error::LibError(Box::new(value))
    }
}