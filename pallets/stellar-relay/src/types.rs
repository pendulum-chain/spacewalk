use crate::{
	traits::{Organization, Validator},
	Config,
};

pub type OrganizationIdOf<T> = <T as Config>::OrganizationId;
pub type ValidatorOf<T> = Validator<OrganizationIdOf<T>>;
pub type OrganizationOf<T> = Organization<OrganizationIdOf<T>>;
