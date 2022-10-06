use crate::{
	traits::{Organization, Validator},
	Config,
};

pub(crate) type OrganizationIdOf<T> = <T as Config>::OrganizationId;
pub(crate) type ValidatorOf<T> = Validator<OrganizationIdOf<T>>;
pub(crate) type OrganizationOf<T> = Organization<OrganizationIdOf<T>>;
