# Webhook 通知详情变量

自定义 JSON 或文本请求体支持 `{{message.fact_values.字段名}}`。编辑器的「通知详情」按事件列出所有可插入字段，「公共详情」列出事件类型、风险级别、来源、发生时间和聚合统计。

```json
{
  "credential": "{{message.fact_values.credential_name}}",
  "totp": "{{message.fact_values.linked_totp}}",
  "comment": "{{message.fact_values.session_comment}}",
  "ip": "{{message.fact_values.login_ip}}",
  "location": "{{message.fact_values.ip_location}}",
  "logout_source": "{{message.fact_values.logout_source}}"
}
```

这些字段的值与通知 `facts[].value` 完全一致，包含本地化文字、格式化时间和会话备注。字段名固定，不随语言或数组顺序变化；按现有详情翻译键转换为 snake_case。例如 `linkedTotp` 对应 `linked_totp`，原始事件中的对应字段仍为 `event.payload.linked_totp_name`。

`{{message.fact_values}}` 可引用整个详情对象。JSON 字符串只含一个变量时保留对象类型，混合文本或文本请求体则转为字符串。`event.payload.*` 仍用于读取原始事件数据。

字段存在但没有内容时返回空字符串；事件不提供的字段会列入预览的缺失变量。独立 JSON 缺失变量返回 `null`，嵌入文本时返回空字符串，不阻止发送。样例 Context 留空使用服务端样例；点击「填入样例 JSON」后可编辑为目标事件的数据。填入的内容用于标准和自定义请求体的预览及测试，不会保存，也不会影响实际投递。

新生成的消息在标准 Webhook 和消息快照中包含 `fact_values`，重试使用原消息快照。历史消息可能没有此字段，旧模板仍可使用；对历史消息引用新变量会显示缺失，不根据旧的翻译标签推测字段。
