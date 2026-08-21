use super::*;

pub(crate) type RedisResult<T> = StorageResult<T>;
#[allow(dead_code)]
pub(crate) type RedisError = StorageError;

#[allow(dead_code)]
pub(crate) trait AsyncCommands {}

#[derive(Clone)]
pub(crate) struct ConnectionManager {
    pub(super) db: Connection,
    pub(super) analytics_db: Connection,
    pub(super) analytics_checkpoint_gate: Arc<RwLock<()>>,
    #[cfg(test)]
    pub(super) path: PathBuf,
}

impl AsyncCommands for ConnectionManager {}

pub(crate) mod streams {
    use super::*;

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamReadOptions {
        pub(crate) count: Option<usize>,
    }

    impl StreamReadOptions {
        pub(crate) fn count(mut self, count: usize) -> Self {
            self.count = Some(count);
            self
        }
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamRangeReply {
        pub(crate) ids: Vec<StreamId>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamReadReply {
        pub(crate) keys: Vec<StreamKey>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamKey {
        #[allow(dead_code)]
        pub(crate) key: String,
        pub(crate) ids: Vec<StreamId>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamId {
        pub(crate) id: String,
        fields: HashMap<String, String>,
    }

    impl StreamId {
        pub(crate) fn new(id: String, fields: HashMap<String, String>) -> Self {
            Self { id, fields }
        }

        pub(crate) fn get<T: FromStreamField>(&self, field: &str) -> Option<T> {
            self.fields
                .get(field)
                .and_then(|value| T::from_field(value))
        }
    }

    pub(crate) trait FromStreamField: Sized {
        fn from_field(value: &str) -> Option<Self>;
    }

    impl FromStreamField for String {
        fn from_field(value: &str) -> Option<Self> {
            Some(value.to_string())
        }
    }
}

pub(crate) enum CmdOutput {
    Nil,
    Int(i64),
    String(String),
    OptionalString(Option<String>),
    Strings(Vec<String>),
    OptionalStrings(Vec<Option<String>>),
    StringPairs(Vec<String>),
    ZPairs(Vec<(String, f64)>),
    StreamEntries(Vec<(String, Vec<String>)>),
    Scan(String, Vec<String>),
    Ints(Vec<i64>),
}

pub(crate) trait FromCmdOutput: Sized {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self>;
}

impl FromCmdOutput for () {
    fn from_cmd_output(_: CmdOutput) -> RedisResult<Self> {
        Ok(())
    }
}

impl FromCmdOutput for i64 {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Int(value) => Ok(value),
            _ => Err(storage_error("unexpected integer command result")),
        }
    }
}

impl FromCmdOutput for usize {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Int(value) => Ok(value.max(0) as usize),
            _ => Err(storage_error("unexpected usize command result")),
        }
    }
}

impl FromCmdOutput for String {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::String(value) => Ok(value),
            CmdOutput::OptionalString(Some(value)) => Ok(value),
            _ => Err(storage_error("unexpected string command result")),
        }
    }
}

impl FromCmdOutput for Option<String> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::OptionalString(value) => Ok(value),
            CmdOutput::String(value) => Ok(Some(value)),
            CmdOutput::Nil => Ok(None),
            _ => Err(storage_error("unexpected optional string command result")),
        }
    }
}

impl FromCmdOutput for Vec<String> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Strings(value) | CmdOutput::StringPairs(value) => Ok(value),
            _ => Err(storage_error("unexpected string vector command result")),
        }
    }
}

impl FromCmdOutput for Vec<Option<String>> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::OptionalStrings(value) => Ok(value),
            _ => Err(storage_error(
                "unexpected optional string vector command result",
            )),
        }
    }
}

impl FromCmdOutput for Vec<i64> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Ints(value) => Ok(value),
            _ => Err(storage_error("unexpected integer vector command result")),
        }
    }
}

impl FromCmdOutput for (String, Vec<String>) {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Scan(cursor, keys) => Ok((cursor, keys)),
            _ => Err(storage_error("unexpected scan command result")),
        }
    }
}

impl FromCmdOutput for Vec<(String, f64)> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::ZPairs(value) => Ok(value),
            _ => Err(storage_error("unexpected zset pair command result")),
        }
    }
}

impl FromCmdOutput for Vec<(String, Vec<String>)> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::StreamEntries(value) => Ok(value),
            _ => Err(storage_error("unexpected stream command result")),
        }
    }
}

pub(crate) trait FromPipeOutput: Sized {
    fn from_pipe_outputs(outputs: Vec<CmdOutput>) -> RedisResult<Self>;
}

impl FromPipeOutput for () {
    fn from_pipe_outputs(_: Vec<CmdOutput>) -> RedisResult<Self> {
        Ok(())
    }
}

impl FromPipeOutput for Vec<i64> {
    fn from_pipe_outputs(outputs: Vec<CmdOutput>) -> RedisResult<Self> {
        outputs
            .into_iter()
            .map(|output| match output {
                CmdOutput::Int(value) => Ok(value),
                _ => Err(storage_error("unexpected pipeline integer result")),
            })
            .collect()
    }
}

pub(crate) trait FromOptionalString: Sized {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self>;
}

impl FromOptionalString for Option<String> {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self> {
        Ok(value)
    }
}

impl FromOptionalString for String {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self> {
        value.ok_or_else(|| storage_error("missing string value"))
    }
}

pub(crate) trait FromDeleteCount: Sized {
    fn from_delete_count(value: usize) -> Self;
}

impl FromDeleteCount for () {
    fn from_delete_count(_: usize) -> Self {}
}

impl FromDeleteCount for usize {
    fn from_delete_count(value: usize) -> Self {
        value
    }
}

impl FromDeleteCount for i64 {
    fn from_delete_count(value: usize) -> Self {
        value as i64
    }
}

pub(crate) trait IntoKey {
    fn into_key(self) -> String;
}

impl IntoKey for &str {
    fn into_key(self) -> String {
        self.to_string()
    }
}

impl IntoKey for String {
    fn into_key(self) -> String {
        self
    }
}

impl IntoKey for &String {
    fn into_key(self) -> String {
        self.clone()
    }
}

pub(crate) trait IntoKeys {
    fn into_keys(self) -> Vec<String>;
}

impl<T: IntoKey> IntoKeys for T {
    fn into_keys(self) -> Vec<String> {
        vec![self.into_key()]
    }
}

impl IntoKeys for &[String] {
    fn into_keys(self) -> Vec<String> {
        self.to_vec()
    }
}

impl IntoKeys for &Vec<String> {
    fn into_keys(self) -> Vec<String> {
        self.clone()
    }
}

impl IntoKeys for Vec<String> {
    fn into_keys(self) -> Vec<String> {
        self
    }
}

impl<const N: usize> IntoKeys for &[&str; N] {
    fn into_keys(self) -> Vec<String> {
        self.iter().map(|value| (*value).to_string()).collect()
    }
}

pub(crate) trait IntoMembers {
    fn into_members(self) -> Vec<String>;
}

impl<T: IntoKey> IntoMembers for T {
    fn into_members(self) -> Vec<String> {
        vec![self.into_key()]
    }
}

impl IntoMembers for &[String] {
    fn into_members(self) -> Vec<String> {
        self.to_vec()
    }
}

impl IntoMembers for &Vec<String> {
    fn into_members(self) -> Vec<String> {
        self.clone()
    }
}

impl IntoMembers for Vec<String> {
    fn into_members(self) -> Vec<String> {
        self
    }
}

pub(crate) trait ToRedisArgs {
    fn append_args(&self, args: &mut Vec<String>);
}

macro_rules! impl_display_arg {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ToRedisArgs for $ty {
                fn append_args(&self, args: &mut Vec<String>) {
                    args.push(self.to_string());
                }
            }
        )*
    };
}

impl_display_arg!(i64, i32, isize, usize, u64, u32, f64);

impl ToRedisArgs for &str {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push((*self).to_string());
    }
}

impl ToRedisArgs for String {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push(self.clone());
    }
}

impl ToRedisArgs for &String {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push((*self).clone());
    }
}

impl ToRedisArgs for &[String] {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for &Vec<String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for Vec<String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for Vec<&String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().map(|value| (*value).clone()));
    }
}

impl<const N: usize> ToRedisArgs for &[&str; N] {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().map(|value| (*value).to_string()));
    }
}
