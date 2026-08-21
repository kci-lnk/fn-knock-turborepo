use super::*;

pub(crate) struct Cmd {
    spec: CommandSpec,
}

pub(crate) fn cmd(name: &str) -> Cmd {
    Cmd {
        spec: CommandSpec::new(name),
    }
}

impl Cmd {
    pub(crate) fn arg<T: ToRedisArgs>(mut self, value: T) -> Self {
        value.append_args(&mut self.spec.args);
        self
    }

    pub(crate) async fn query_async<T: FromCmdOutput>(
        self,
        conn: &mut ConnectionManager,
    ) -> RedisResult<T> {
        T::from_cmd_output(conn.execute_command(self.spec).await?)
    }
}

pub(crate) struct Pipeline {
    commands: Vec<CommandSpec>,
    current: Option<CommandSpec>,
}

pub(crate) fn pipe() -> Pipeline {
    Pipeline {
        commands: Vec::new(),
        current: None,
    }
}

impl Pipeline {
    pub(crate) fn cmd(&mut self, name: &str) -> &mut Self {
        self.flush_current();
        self.current = Some(CommandSpec::new(name));
        self
    }

    pub(crate) fn arg<T: ToRedisArgs>(&mut self, value: T) -> &mut Self {
        if let Some(current) = &mut self.current {
            value.append_args(&mut current.args);
        }
        self
    }

    pub(crate) fn ignore(&mut self) -> &mut Self {
        if let Some(current) = &mut self.current {
            current.ignore = true;
        } else if let Some(last) = self.commands.last_mut() {
            last.ignore = true;
        }
        self.flush_current();
        self
    }

    pub(crate) fn set<K: IntoKey, V: Display>(&mut self, key: K, value: V) -> &mut Self {
        self.push_simple("SET", vec![key.into_key(), value.to_string()])
    }

    pub(crate) fn set_ex<K: IntoKey, V: Display>(
        &mut self,
        key: K,
        value: V,
        ttl_seconds: u64,
    ) -> &mut Self {
        self.push_simple(
            "SETEX",
            vec![key.into_key(), ttl_seconds.to_string(), value.to_string()],
        )
    }

    pub(crate) fn del<K: IntoKeys>(&mut self, keys: K) -> &mut Self {
        self.push_simple("DEL", keys.into_keys())
    }

    pub(crate) fn hset<K: IntoKey, F: Display, V: Display>(
        &mut self,
        key: K,
        field: F,
        value: V,
    ) -> &mut Self {
        self.push_simple(
            "HSET",
            vec![key.into_key(), field.to_string(), value.to_string()],
        )
    }

    pub(crate) fn hset_multiple(&mut self, key: &str, values: &[(&String, &String)]) -> &mut Self {
        let mut args = vec![key.to_string()];
        for (field, value) in values {
            args.push((*field).clone());
            args.push((*value).clone());
        }
        self.push_simple("HSET", args)
    }

    pub(crate) fn hdel<K: IntoKey, F: IntoMembers>(&mut self, key: K, fields: F) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(fields.into_members());
        self.push_simple("HDEL", args)
    }

    pub(crate) fn sadd<K: IntoKey, M: IntoMembers>(&mut self, key: K, members: M) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(members.into_members());
        self.push_simple("SADD", args)
    }

    pub(crate) fn srem<K: IntoKey, M: IntoMembers>(&mut self, key: K, members: M) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(members.into_members());
        self.push_simple("SREM", args)
    }

    pub(crate) fn zadd<K: IntoKey, M: Display, S: Display>(
        &mut self,
        key: K,
        member: M,
        score: S,
    ) -> &mut Self {
        self.push_simple(
            "ZADD",
            vec![key.into_key(), score.to_string(), member.to_string()],
        )
    }

    pub(crate) fn zrem<K: IntoKey, M: IntoMembers>(&mut self, key: K, members: M) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(members.into_members());
        self.push_simple("ZREM", args)
    }

    pub(crate) fn zrembyscore<K: IntoKey, Min: Display, Max: Display>(
        &mut self,
        key: K,
        min_score: Min,
        max_score: Max,
    ) -> &mut Self {
        self.push_simple(
            "ZREMRANGEBYSCORE",
            vec![key.into_key(), min_score.to_string(), max_score.to_string()],
        )
    }

    pub(crate) fn zcard<K: IntoKey>(&mut self, key: K) -> &mut Self {
        self.push_simple("ZCARD", vec![key.into_key()])
    }

    pub(crate) fn ttl<K: IntoKey>(&mut self, key: K) -> &mut Self {
        self.push_simple("TTL", vec![key.into_key()])
    }

    pub(crate) fn expire<K: IntoKey, T: Display>(&mut self, key: K, ttl_seconds: T) -> &mut Self {
        self.push_simple("EXPIRE", vec![key.into_key(), ttl_seconds.to_string()])
    }

    pub(crate) async fn query_async<T: FromPipeOutput>(
        mut self,
        conn: &mut ConnectionManager,
    ) -> RedisResult<T> {
        self.flush_current();
        T::from_pipe_outputs(conn.execute_pipeline(self.commands).await?)
    }

    /// Executes this pipeline inside a caller-owned SQLite transaction.
    ///
    /// Domain repositories use this together with
    /// [`hash_field_matches_in_transaction`] so a typed-table mutation and
    /// its compatibility-keyspace indexes share one commit boundary.
    pub(crate) fn query_in_transaction<T: FromPipeOutput>(
        mut self,
        tx: &rusqlite::Transaction<'_>,
    ) -> RedisResult<T> {
        self.flush_current();
        T::from_pipe_outputs(execute_pipeline_commands_tx(tx, self.commands)?)
    }

    pub(crate) async fn query_async_replacing_prefix<T: FromPipeOutput>(
        mut self,
        conn: &mut ConnectionManager,
        prefix: &str,
    ) -> RedisResult<(usize, T)> {
        self.flush_current();
        let (deleted, outputs) = conn
            .execute_pipeline_replacing_prefix(prefix, self.commands)
            .await?;
        Ok((deleted, T::from_pipe_outputs(outputs)?))
    }

    fn push_simple(&mut self, name: &str, args: Vec<String>) -> &mut Self {
        self.flush_current();
        self.commands.push(CommandSpec {
            name: name.to_ascii_uppercase(),
            args,
            ignore: false,
        });
        self
    }

    fn flush_current(&mut self) {
        if let Some(current) = self.current.take() {
            self.commands.push(current);
        }
    }
}
