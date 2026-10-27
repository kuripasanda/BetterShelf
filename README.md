# BookShelf
マインクラフトのプラグインサーバー（Paper / Folia）向けのプラグインを管理するツールです。
Modrinth / Hangar / Spigot(SpigotMC) からプラグインを検索・導入・更新できます。

## インストール (Ubuntu / Linux)
[Releases](https://github.com/kuripasanda/bookshelf/releases) からバイナリを取得します。静的リンク済みなので追加ライブラリは不要です。

```bash
# x86_64 の場合 (ARM は aarch64 に読み替え)
curl -L -o bookshelf https://github.com/kuripasanda/bookshelf/releases/latest/download/bookshelf-linux-x86_64
chmod +x bookshelf
sudo mv bookshelf /usr/local/bin/      # または ~/.local/bin/
```
`./bookshelf plugins check` のようにパス指定で直接実行することもできます。
更新は `bookshelf self-update` で行えます。

## はじめかた
```bash
cd /path/to/server           # サーバーのルート
bookshelf init               # bookshelf.yml を作成 (ソフトウェア / MCバージョンは自動検出)
bookshelf plugins search luckperms
bookshelf plugins install modrinth:luckperms
```

## コマンド
| コマンド | 説明 |
| -- | -- |
| `bookshelf init` | サーバーに `bookshelf.yml` を作成します。 |
| `bookshelf plugins search <キーワード>` | Modrinth / Hangar / Spigot を横断検索します。`--source` で絞り込み。 |
| `bookshelf plugins list` | 登録済みプラグインを一覧表示します。 |
| `bookshelf plugins register` | 未登録の jar をすべて登録します。 |
| `bookshelf plugins check` | 全てのプラグインの更新を確認します。 |
| `bookshelf plugins install <source>:<id>` | プラグインを導入します。例: `modrinth:luckperms` `hangar:ViaVersion` `spigot:28140` |
| `bookshelf plugins update [名前...]` | 更新します (省略で全て)。旧 jar は `.bookshelf/backup/` に退避されます。 |
| `bookshelf plugins remove <名前>` | jar を退避して登録を解除します。 |
| `bookshelf self-update` | bookshelf 自身を最新版にします。 |

共通オプション: `--dir <path>` (サーバーのディレクトリ) / `-y, --yes` (確認を省略) / `--no-color`。
`search` `list` `check` は `--json` で機械可読な出力ができます。`NO_COLOR` 環境変数にも対応しています。

## bookshelf.yml
```yaml
server:
  software: paper        # paper | folia
  minecraft: 1.21.4      # 更新確認時の絞り込みに使用
  plugins_dir: plugins
plugins:
- name: LuckPerms
  source: modrinth       # modrinth | hangar | spigot
  id: luckperms
  version: 5.5.71
  file: LuckPerms-Bukkit-5.5.71.jar
  sha256: ...
  pin: true              # 任意。true にすると update の対象外
```

## ソースごとの注意
- **Modrinth**: jar のハッシュで確実に照合できます。
- **Hangar**: 名前で検索して登録します。SHA-256 が一致するバージョンがあればそのバージョンを採用します。
- **Spigot**: 公式 API が無いため [Spiget](https://spiget.org) を利用します。SpigotMC が直接ホストするファイルは自動ダウンロードできないため、更新の有無とページ URL の案内のみ行います。

## 開発
```bash
cargo test
cargo build --release --target x86_64-unknown-linux-musl   # 静的バイナリ
```
`v*` タグを push すると GitHub Actions が x86_64 / aarch64 のバイナリと SHA-256 を Release に添付します。
