# ferncad 開発環境セットアップ

## 前提条件

- Docker がインストールされていること

## イメージのビルド

```bash
docker build -f .devcontainer/Dockerfile -t ferncad-dev .devcontainer/
```

## コンテナに入る

```bash
docker run -it --rm \
  -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -v $(pwd):/workspace \
  ferncad-dev bash
```

`HOST_UID` / `HOST_GID` により、コンテナ内で作成したファイルの所有者が
ホスト側のユーザーと一致します。省略時は `1000:1000` がデフォルトです。

## コンテナ内で Claude Code を使う

Claude Code はプロジェクト本体に依存しないため、個人の開発環境として
`.devcontainer/personal/` に配置されています（`.gitignore` 対象）。

### 初回セットアップ

コンテナ内で以下を実行:

```bash
# Claude Code ネイティブインストール
curl -fsSL https://claude.ai/install.sh | bash
export PATH="$HOME/.local/bin:$PATH"
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

### Claude Code の起動

```bash
# 対話モード
ANTHROPIC_API_KEY="sk-ant-..." claude

# 権限確認なしで自走（コンテナ内なので安全）
ANTHROPIC_API_KEY="sk-ant-..." claude --dangerously-skip-permissions

# 非対話モード（Issue を読ませて実装させる等）
ANTHROPIC_API_KEY="sk-ant-..." claude -p "Phase 1 タスク 1-1 を実装してください" --dangerously-skip-permissions
```

### ワンライナーで Claude Code を起動

```bash
docker run -it --rm \
  -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
  -e ANTHROPIC_API_KEY="sk-ant-..." \
  -v $(pwd):/workspace \
  ferncad-dev bash -c '
    curl -fsSL https://claude.ai/install.sh | bash
    export PATH="$HOME/.local/bin:$PATH"
    claude --dangerously-skip-permissions
  '
```

## devcontainer CLI / VS Code で使う場合

`devcontainer.json` が設定済みのため、VS Code の「Reopen in Container」
またはdevcontainer CLIで起動すれば自動的に環境が構築されます。

UID/GID はホスト側の値が `containerEnv` 経由で自動的に渡されます。

## 別の LLM ツールを使う場合

`.devcontainer/personal/` に自分のセットアップスクリプトを配置してください。
このディレクトリは `.gitignore` 対象のため、他の開発者に影響しません。
