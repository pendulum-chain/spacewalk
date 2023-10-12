use frame_support::BoundedVec;
use crate::{
	traits::{Organization, Validator},
	Config,
};

pub type OrganizationIdOf<T> = <T as Config>::OrganizationId;
pub type ValidatorOf<T> = Validator<OrganizationIdOf<T>>;
pub type OrganizationOf<T> = Organization<OrganizationIdOf<T>>;

pub type ValidatorsList<T> = BoundedVec<ValidatorOf<T>, <T as Config>::ValidatorLimit>;
pub type OrganizationsList<T> = BoundedVec<OrganizationOf<T>, <T as Config>::OrganizationLimit>;