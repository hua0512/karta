# Admin Commands

Admins configured in `kovi.conf.toml` (`main_admin` and `admins`) can manage Karta directly from QQ chat. Unrecognized `/`-prefixed commands are ignored silently so Karta coexists cleanly with other bots in the same group.

All runtime changes are persisted to `data/karta/state.json` and survive restarts ([Runtime State](configuration.md#runtime-state)).

## Works in any chat (group or private)

| Command | Description |
|---|---|
| `/status` | Show render mode, all 16 platforms' global status (plus cookie state for the 7 primary cookie-configured platforms), tool status, access control, and runtime counts |
| `/login bilibili` | Start Bilibili QR-code login (alias: `/login bili`). Sends a QR image and polls for up to 120 s |
| `/ban <uid>` | Globally ban a user by numeric QQ ID |
| `/unban <uid>` | Remove a global ban |

::: info What `/status` reports
- **Platform cookies:** Reports credential/cookie state for the 7 primary cookie-configured platforms (`bilibili`, `youtube`, `xiaohongshu`, `twitter`, `weibo`, `xiaoheihe`, `douyin`).
- **Tools:** Probes whether `ffmpeg` is available on `PATH` / `ffmpeg_path`, and reports `yt-dlp` as `configured` when either `ytdlp_path` or `ytdlp_cookies_file` is set in `config.json`.
:::

## Group chat only

Sending these in a private chat returns a reminder that they must be run inside a group.

| Command | Description |
|---|---|
| `/enable <platform>` | Re-enable a platform in the current group |
| `/disable <platform>` | Disable a platform in the current group |
| `/karta on` | Enable Karta in the current group |
| `/karta off` | Disable Karta in the current group |

::: warning Global disables take precedence
`/enable <platform>` cannot override a platform listed in `disabled_platforms` in `config.json` — remove it from `config.json` and restart to enable it.
:::

## `/karta on|off` semantics

The group toggle follows `group_blacklist_enabled` in `config.json`:

- **Blacklist mode** (default, `true`): every group starts enabled; `/karta off` disables parsing in that group.
- **Whitelist mode** (`false`): every group starts disabled; `/karta on` enables parsing in that group.

Both modes share the same persisted group set in `state.json`, so flipping `group_blacklist_enabled` inverts any groups already toggled — re-run `/karta on|off` after switching modes. Private chats are unaffected and always parse.

## Platform names and aliases

`/enable` and `/disable` accept any platform's canonical lowercase slug (`bilibili`, `douyin`, `twitter`, `youtube`, `tiktok`, `weibo`, `xiaohongshu`, `kuaishou`, `hongguo`, `acfun`, `nga`, `xiaoheihe`, `instagram`, `v2ex`, `wechat`, `wechatchannel`) as well as these case-insensitive aliases:

| Platform | Slug | Aliases |
|---|---|---|
| Bilibili | `bilibili` | `bili`, `b站` |
| Douyin | `douyin` | `抖音` |
| Twitter/X | `twitter` | `x`, `推特` |
| YouTube | `youtube` | `yt` |
| Weibo | `weibo` | `微博` |
| Xiaohongshu | `xiaohongshu` | `xhs`, `小红书` |
| Kuaishou | `kuaishou` | `ks`, `快手` |
| Hongguo | `hongguo` | `hg`, `红果`, `红果短剧` |
| Xiaoheihe | `xiaoheihe` | `xhh`, `heybox`, `小黑盒` |
| Instagram | `instagram` | `ig` |
| V2EX | `v2ex` | `v站` |
| WeChat Articles | `wechat` | `wx`, `mp`, `微信`, `公众号`, `微信公众号` |
| WeChat Channels | `wechatchannel` | `sph`, `wxchannel`, `视频号`, `微信视频号` |
