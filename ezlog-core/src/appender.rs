use std::{
    fs::OpenOptions,
    io::{BufReader, BufWriter},
    path::PathBuf,
    rc::Rc,
};
use time::OffsetDateTime;

use crate::*;

pub trait AppenderInner: Write {
    fn is_oversize(&self, buf_size: usize) -> bool;

    fn is_overtime(&self, time: OffsetDateTime) -> bool;

    fn file_path(&self) -> &PathBuf;
}

/// # Appender 的实现
pub struct EZAppender {
    config: Rc<EZLogConfig>,
    inner: Box<dyn AppenderInner>,
}

impl EZAppender {
    pub fn create_inner(config: &EZLogConfig) -> Result<Box<dyn AppenderInner>> {
        Self::create_inner_by_time(config, OffsetDateTime::now_utc())
    }

    pub fn create_inner_by_time(
        config: &EZLogConfig,
        time: OffsetDateTime,
    ) -> Result<Box<dyn AppenderInner>> {
        if let Ok(inner) = MmapAppendInner::new(config, time) {
            return Ok(Box::new(inner));
        } else {
            //todo mmap create error
        }

        Ok(Box::new(ByteArrayAppenderInner::new(config, time)?))
    }

    pub fn new(config: Rc<EZLogConfig>) -> Result<Self> {
        let inner = EZAppender::create_inner(&config)?;
        Ok(Self { config, inner })
    }

    fn check_rolling(&mut self, buf_size: usize) -> Result<()> {
        self.check_recreate_inner(OffsetDateTime::now_utc(), buf_size)
    }

    fn check_recreate_inner(&mut self, time: OffsetDateTime, buf_size: usize) -> Result<()> {
        if self.inner.is_overtime(time) {
            self.inner = Self::create_inner_by_time(&self.config, time)?;
        }

        if self.inner.is_oversize(buf_size) {
            // drop current inner, then rename the log file
            self.inner = Box::new(NopInner::empty());
            rename_current_file(self.inner.file_path())?;

            self.inner = Self::create_inner_by_time(&self.config, time)?;
        }
        Ok(())
    }
}

impl Write for EZAppender {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.check_rolling(buf.len())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) struct MmapAppendInner {
    header: Header,
    file_path: PathBuf,
    mmap: MmapMut,
    next_date: i64,
}

impl MmapAppendInner {
    pub(crate) fn new(config: &EZLogConfig, time: OffsetDateTime) -> Result<Self> {
        let (mut file_path, mut mmap) = config.create_mmap_file(time)?;
        let mut c = Cursor::new(&mut mmap[0..V1_LOG_HEADER_SIZE]);
        let mut header = Header::decode(&mut c).unwrap_or_else(|_| Header::new());
        let next_date = next_date(time);

        let mut write_header = false;
        if header.is_empty() {
            header = Header::create(config);
            write_header = true;
        } else if !header.is_empty() && !header.is_valid(config) {
            rename_current_file(&file_path)?;
            (file_path, mmap) = config.create_mmap_file(time)?;
            header = Header::create(config);
            write_header = true;
        }

        let mut inner = MmapAppendInner {
            header,
            file_path,
            mmap,
            next_date: next_date.unix_timestamp(),
        };
        if write_header {
            inner.write_header()?;
        }
        Ok(inner)
    }

    pub(crate) fn new_now(config: &EZLogConfig) -> Result<Self> {
        MmapAppendInner::new(config, OffsetDateTime::now_utc())
    }

    fn write_header(&mut self) -> std::result::Result<(), std::io::Error> {
        let mut c = Cursor::new(&mut self.mmap[0..V1_LOG_HEADER_SIZE]);
        self.header.encode(&mut c)
    }

    fn write_buf(&mut self, buf: &[u8], start: usize) -> std::io::Result<usize> {
        let mut c = Cursor::new(&mut self.mmap[start..start + buf.len()]);
        c.write(buf)
    }
}

impl Write for MmapAppendInner {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let start = self.header.recorder_position as usize;
        self.header.recorder_position += buf.len() as u32;
        self.write_header()?;
        self.write_buf(buf, start)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.write_header()?;
        self.mmap.flush()
    }
}

impl AppenderInner for MmapAppendInner {
    fn is_oversize(&self, buf_size: usize) -> bool {
        let max_len = self.mmap.len();
        self.header.recorder_position as usize + buf_size > max_len
    }

    fn is_overtime(&self, time: OffsetDateTime) -> bool {
        time.unix_timestamp() > self.next_date
    }

    fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
}

impl Drop for MmapAppendInner {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

struct ByteArrayAppenderInner {
    header: Header,
    file_path: PathBuf,
    byte_array: Vec<u8>,
    next_date: i64,
}

impl ByteArrayAppenderInner {
    pub(crate) fn new(config: &EZLogConfig, time: OffsetDateTime) -> Result<Self> {
        let (mut _file, mut file_path) = config.create_log_file(time)?;
        let mut byte_array = vec![0u8; config.max_size as usize];
        BufReader::new(&_file).read_exact(&mut byte_array)?;

        let mut c = Cursor::new(&mut byte_array[0..V1_LOG_HEADER_SIZE]);
        let mut header = Header::decode(&mut c).unwrap_or_else(|_| Header::new());
        let next_date = next_date(time);

        let mut write_header = false;
        if header.is_empty() {
            header = Header::create(config);
            write_header = true;
        } else if !header.is_empty() && !header.is_valid(config) {
            rename_current_file(&file_path)?;
            (_file, file_path) = config.create_log_file(time)?;
            header = Header::create(config);
            write_header = true;
        }

        let mut inner = ByteArrayAppenderInner {
            header,
            file_path,
            byte_array,
            next_date: next_date.unix_timestamp(),
        };
        if write_header {
            inner.write_header()?;
        }
        Ok(inner)
    }

    fn write_header(&mut self) -> std::result::Result<(), std::io::Error> {
        let mut c = Cursor::new(&mut self.byte_array[0..V1_LOG_HEADER_SIZE]);
        self.header.encode(&mut c)
    }

    fn write_buf(&mut self, buf: &[u8], start: usize) {
        (&mut self.byte_array[start..start + buf.len()]).copy_from_slice(buf)
    }
}

impl Write for ByteArrayAppenderInner {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let start = self.header.recorder_position as usize;
        self.header.recorder_position += buf.len() as u32;
        self.write_header()?;
        self.write_buf(buf, start);
        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.write_header()?;
        let file = OpenOptions::new().write(true).open(self.file_path())?;
        let mut write = BufWriter::new(file);
        write.write_all(&self.byte_array)?;
        write.flush()
    }
}

impl AppenderInner for ByteArrayAppenderInner {
    fn is_oversize(&self, buf_size: usize) -> bool {
        let max_len = self.byte_array.len();
        self.header.recorder_position as usize + buf_size > max_len
    }

    fn is_overtime(&self, time: OffsetDateTime) -> bool {
        time.unix_timestamp() > self.next_date
    }

    fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
}

impl Drop for ByteArrayAppenderInner {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

pub fn rename_current_file(file_path: &PathBuf) -> Result<()> {
    let mut count = 1;
    loop {
        if let Some(ext) = file_path.extension() {
            let new_ext = format!("{}.{}", count, ext.to_str().unwrap_or("mmap"));
            let new_path = file_path.with_extension(new_ext);
            if !new_path.exists() {
                std::fs::rename(file_path, &new_path)?;
                return Ok(());
            }
        }
        count += 1;
    }
}

struct NopInner {
    file_path: PathBuf,
}

impl NopInner {
    pub(crate) fn empty() -> Self {
        NopInner {
            file_path: PathBuf::new(),
        }
    }
}

impl AppenderInner for NopInner {
    fn is_oversize(&self, _buf_size: usize) -> bool {
        false
    }

    fn is_overtime(&self, _time: OffsetDateTime) -> bool {
        false
    }

    fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
}

impl Write for NopInner {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::fs::{File, OpenOptions};
    use std::io::{BufReader, Seek, SeekFrom};

    use crate::config::EZLogConfigBuilder;

    use super::*;

    #[test]
    fn create_all_feature_config() {
        let key = b"an example very very secret key.";
        let nonce = b"unique nonce";
        EZLogConfigBuilder::new()
            .dir_path(
                dirs::desktop_dir()
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            )
            .name(String::from("all_feature"))
            .file_suffix(String::from("mmap"))
            .max_size(1024)
            .compress(CompressKind::ZLIB)
            .cipher(CipherKind::AES128GCM)
            .cipher_key(key.to_vec())
            .cipher_nonce(nonce.to_vec())
            .build();
    }

    #[test]
    fn test_appender_inner_rolling() {
        let config = EZLogConfigBuilder::new()
            .name("test".to_string())
            .dir_path(
                dirs::desktop_dir()
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            )
            .duration(Duration::days(1))
            .name(String::from("test"))
            .file_suffix(String::from("mmap"))
            .max_size(1024)
            .build();

        let inner = MmapAppendInner::new_now(&config).unwrap();
        assert!(inner.is_oversize(1015));
        assert!(!inner.is_oversize(1014));
        assert!(inner.is_overtime(time::OffsetDateTime::now_utc() + Duration::days(1)));
        assert!(!inner.is_overtime(
            time::OffsetDateTime::now_utc()
                .date()
                .midnight()
                .assume_utc()
                + Duration::hours(23)
        ));
        fs::remove_file(inner.file_path()).unwrap();

        let inner = ByteArrayAppenderInner::new(&config, OffsetDateTime::now_utc()).unwrap();
        assert!(inner.is_oversize(1015));
        assert!(!inner.is_oversize(1014));
        assert!(inner.is_overtime(time::OffsetDateTime::now_utc() + Duration::days(1)));
        assert!(!inner.is_overtime(
            time::OffsetDateTime::now_utc()
                .date()
                .midnight()
                .assume_utc()
                + Duration::hours(23)
        ));
        fs::remove_file(inner.file_path()).unwrap();
    }

    fn current_file(path: &PathBuf) -> std::result::Result<File, errors::LogError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(path)?;
        Ok(file)
    }

    #[test]
    fn test_appender_write() {
        let buf = b"hello an other log, let's go";

        let c = EZLogConfigBuilder::new()
            .dir_path(
                dirs::desktop_dir()
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            )
            .name(String::from("test_write"))
            .file_suffix(String::from("mmap"))
            .max_size(1024)
            .build();

        let config = Rc::new(c);
        let mut appender = MmapAppendInner::new(&config, OffsetDateTime::now_utc()).unwrap();
        appender.write(buf).unwrap();
        appender.flush().unwrap();

        let mut read_buf = vec![0u8; buf.len()];
        let file = current_file(&appender.file_path()).unwrap();
        let mut reader: BufReader<File> = BufReader::new(file);
        reader
            .seek(SeekFrom::Start(V1_LOG_HEADER_SIZE as u64))
            .unwrap();
        reader.read(&mut read_buf).unwrap();

        assert_eq!(read_buf, buf);
        let p = appender.file_path().clone();
        drop(appender);
        fs::remove_file(p).unwrap();

        let c = EZLogConfigBuilder::new()
            .dir_path(
                dirs::desktop_dir()
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            )
            .name(String::from("test_write1"))
            .file_suffix(String::from("mmap"))
            .max_size(1024)
            .build();

        let mut appender = ByteArrayAppenderInner::new(&c, OffsetDateTime::now_utc()).unwrap();
        appender.write(buf).unwrap();
        appender.flush().unwrap();

        let log_path = appender.file_path().clone();

        let mut read_buf = vec![0u8; buf.len()];
        let file = current_file(&log_path).unwrap();
        let mut reader = BufReader::new(file);
        reader
            .seek(SeekFrom::Start(V1_LOG_HEADER_SIZE as u64))
            .unwrap();
        reader.read_exact(&mut read_buf).unwrap();
        assert_eq!(read_buf, buf);
        fs::remove_file(appender.file_path()).unwrap();
    }
}
