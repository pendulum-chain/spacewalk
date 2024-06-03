use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::{cmp::Ord, fmt::Debug};

/// Enum indicating the status of the Spacewalk Parachain.
#[cfg_attr(feature = "std", derive(serde::Serialize))]
#[derive(Default, Encode, Decode, Clone, Copy, PartialEq, MaxEncodedLen, Eq, Debug, TypeInfo, serde::Deserialize)]
pub enum StatusCode {
	/// Spacewalk Parachain is fully operational.
	#[default]
	Running,
	/// An error has occurred. See Errors for more details.
	Error,
	/// Spacewalk Parachain operation has been fully suspended.
	Shutdown,
}

/// Enum specifying errors which lead to the Error status, tacked in Errors
#[derive(
	Default, Encode, Decode, Clone, PartialEq, Eq, Debug, Ord, MaxEncodedLen, PartialOrd, TypeInfo,
)]
pub enum ErrorCode {
	/// No error. Used as default value
	#[default]
	None,
	OracleOffline,
}

#[macro_export]
macro_rules! error_set {
    () => { BTreeSet::<ErrorCode>::new() };
    ($($x:expr),*) => {
        {
            let mut set = BTreeSet::<ErrorCode>::new();
            $(
                set.insert($x);
            )*
            set
        }
    };
}
