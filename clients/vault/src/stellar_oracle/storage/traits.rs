use crate::stellar_oracle::{Error, Filename, SerializedData, Slot};
use std::{
    fs,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    str::Split,
};

pub trait FileHandlerExt<T: Default>: FileHandler<T> {
    fn create_filename_and_data(data: &T) -> Result<(Filename, SerializedData), Error>;

    fn write_to_file(data: &T) -> Result<Filename, Error> {
        let (filename, data) = Self::create_filename_and_data(data)?;

        let path = Self::get_path(&filename);
        let mut file = File::create(path)?;

        file.write_all(&data)?;

        Ok(filename)
    }
}

pub trait FileHandler<T: Default> {
    // path to where the file should be saved
    const PATH: &'static str;

    fn deserialize_bytes(bytes: Vec<u8>) -> Result<T, Error>;

    fn check_slot_in_splitted_filename(slot_param: Slot, splits: &mut Split<&str>) -> bool;

    fn get_path(filename: &str) -> PathBuf {
        let mut path = PathBuf::new();
        path.push(Self::PATH);
        path.push(filename);
        path
    }

    fn read_file(filename: &str) -> Result<T, Error> {
        let path = Self::get_path(filename);
        let mut file = File::open(path)?;

        let mut bytes: Vec<u8> = vec![];
        let read_size = file.read_to_end(&mut bytes)?;

        if read_size > 0 {
            return Self::deserialize_bytes(bytes);
        }

        Ok(T::default())
    }

    fn find_file_by_slot(slot_param: Slot) -> Result<String, Error> {
        let paths = fs::read_dir(Self::PATH)?;

        for path in paths {
            let filename = path?.file_name().into_string().unwrap();
            let mut splits = filename.split("_");

            if Self::check_slot_in_splitted_filename(slot_param, &mut splits) {
                return Ok(filename);
            }
        }

        Err(Error::Other(format!("Cannot find file for slot {}", slot_param)))
    }

    fn get_map_from_archives(slot: Slot) -> Result<T, Error> {
        let filename = Self::find_file_by_slot(slot)?;

        Self::read_file(&filename)
    }
}
