"""Executed over SSH; JSON on stdin/stdout, Python 3 standard library only.

No configuration values are interpolated into shell commands.
"""
import datetime
import json
import os
import re
import ssl
import stat
import sys
import tempfile
import time
import urllib.request
import urllib.error
from urllib.parse import urlencode, urlsplit

MAX_FILE = 8 * 1024 * 1024
HTTP_TIMEOUT = 20
MAX_HTTP_BODY = 2 * 1024 * 1024
MAX_DIRECTORY_ENTRIES = 512


def environment_status(request):
    names = request.get("names", [])
    if not isinstance(names, list) or len(names) > 128:
        raise ValueError("一次最多检查 128 个环境变量")
    if any(not isinstance(name, str) or not name or "=" in name or "\0" in name for name in names):
        raise ValueError("环境变量名无效")
    return [
        {"name": name, "is_set": bool(os.environ.get(name, "").strip())}
        for name in sorted(set(names))
    ]


def path_kind(path):
    try:
        mode = os.lstat(path).st_mode
    except FileNotFoundError:
        return "missing"
    if stat.S_ISLNK(mode):
        return "symlink"
    if stat.S_ISDIR(mode):
        return "directory"
    if stat.S_ISREG(mode):
        return "file"
    return "other"


def inspect_path(request, browse=False):
    user_home = os.path.expanduser("~")
    home = resolve(user_home, request.get("home") or os.environ.get("CODEX_HOME") or "~/.codex")
    path = resolve(home, request.get("path", "." if browse else ""))
    if not browse:
        return {"path": path, "kind": path_kind(path)}
    if path_kind(path) != "directory":
        raise ValueError("只能浏览普通目录，不跟随目录符号链接")
    # Pin the directory while listing; do not follow a replaced final symlink.
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | getattr(os, "O_NOFOLLOW", 0))
    entries, truncated = [], False
    try:
        with os.scandir(fd) as listing:
            for entry in listing:
                if len(entries) >= MAX_DIRECTORY_ENTRIES:
                    truncated = True
                    break
                kind = "symlink" if entry.is_symlink() else (
                    "directory" if entry.is_dir(follow_symlinks=False) else
                    "file" if entry.is_file(follow_symlinks=False) else "other"
                )
                entries.append({"name": entry.name, "path": os.path.join(path, entry.name), "kind": kind})
    finally:
        os.close(fd)
    entries.sort(key=lambda item: (item["kind"] != "directory", item["name"]))
    return {
        "path": path, "parent": os.path.dirname(path) if path != "/" else None,
        "entries": entries, "truncated": truncated,
    }


def read_file(path):
    try:
        info = os.lstat(path)
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(info.st_mode):
        raise ValueError("拒绝读取符号链接或非普通文件: " + path)
    if info.st_size > MAX_FILE:
        raise ValueError("文件超过 8 MiB: " + path)
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(fd, "rb") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError("不是普通文件: " + path)
        data = stream.read(MAX_FILE + 1)
    if len(data) > MAX_FILE:
        raise ValueError("文件超过 8 MiB: " + path)
    return data.decode("utf-8")


def resolve(home, raw):
    if not isinstance(raw, str) or not raw or "\0" in raw:
        raise ValueError("远程路径不能为空或包含 NUL")
    if raw.startswith("~") and raw != "~" and not raw.startswith("~/"):
        raise ValueError("远程路径仅支持 ~ 或 ~/，不支持 ~user")
    return os.path.abspath(os.path.join(home, os.path.expanduser(raw)))


def unchanged(path, expected):
    if read_file(path) != expected:
        raise ValueError("远程文件已被其他程序修改，请重新载入后再保存: " + path)


def stage(path, text):
    data = text.encode("utf-8")
    if len(data) > MAX_FILE:
        raise ValueError("文件超过 8 MiB: " + path)
    parent = os.path.dirname(path)
    os.makedirs(parent, mode=0o700, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix="." + os.path.basename(path) + ".gui-tmp-", dir=parent)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        os.unlink(temporary)
        raise
    return temporary


def backup(path, text):
    # Exclusive creation: same-second saves never overwrite an earlier backup.
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    base = path + ".bak-" + stamp + "-" + str(time.time_ns() % 1000000000).zfill(9)
    for sequence in range(1000):
        name = base if sequence == 0 else base + "-" + str(sequence)
        try:
            fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        except FileExistsError:
            continue
        with os.fdopen(fd, "wb") as stream:
            stream.write(text.encode("utf-8"))
            stream.flush()
            os.fsync(stream.fileno())
        return name
    raise ValueError("无法创建唯一备份: " + path)


def prune(path):
    parent, name = os.path.split(path)
    pattern = re.compile(re.escape(name) + r"\.bak-(\d{8}-\d{6}-\d{9})(?:-(\d+))?$")
    backups = [
        item for item in os.scandir(parent)
        if pattern.fullmatch(item.name) and item.is_file(follow_symlinks=False)
    ]
    def order(item):
        match = pattern.fullmatch(item.name)
        return match.group(1), int(match.group(2) or "0")
    backups.sort(key=order, reverse=True)
    for item in backups[20:]:
        os.unlink(item.path)


def save(request):
    files = request["files"]
    if not 1 <= len(files) <= 2:
        raise ValueError("只允许保存配置及模型目录")
    paths = [item["path"] for item in files]
    if any(not os.path.isabs(path) or os.path.normpath(path) != path for path in paths):
        raise ValueError("保存路径必须是规范化的绝对路径")
    if len(set(paths)) != len(paths):
        raise ValueError("配置和模型目录不能指向同一文件")
    if len(paths) == 2 and all(os.path.exists(path) for path in paths):
        if os.path.samefile(*paths):
            raise ValueError("配置和模型目录不能指向同一文件")
    for item in files:
        unchanged(item["path"], item["original"])
    changed = [item for item in files if item["write"]]
    stages, backups, published = {}, [], []
    try:
        for item in changed:
            stages[item["path"]] = stage(item["path"], item["text"])
        for item in files:
            unchanged(item["path"], item["original"])
        for item in changed:
            if item["original"] is not None:
                backups.append(backup(item["path"], item["original"]))
        for item in changed:
            path = item["path"]
            unchanged(path, item["original"])
            os.replace(stages[path], path)
            del stages[path]
            published.append(item)
    except BaseException as error:
        rollback_errors = []
        for item in reversed(published):
            path = item["path"]
            try:
                # Do not overwrite a third party's edit while rolling back.
                unchanged(path, item["text"])
                if item["original"] is None:
                    os.unlink(path)
                else:
                    temporary = stage(path, item["original"])
                    try:
                        os.replace(temporary, path)
                    finally:
                        if os.path.exists(temporary):
                            os.unlink(temporary)
            except BaseException as rollback_error:
                rollback_errors.append(str(rollback_error))
        suffix = "; 回滚失败: " + "; ".join(rollback_errors) if rollback_errors else ""
        raise ValueError(str(error) + suffix + ("; 备份: " + ", ".join(backups) if backups else ""))
    finally:
        for temporary in stages.values():
            if os.path.exists(temporary):
                os.unlink(temporary)
    for item in changed:
        try:
            prune(item["path"])
        except OSError:
            pass  # Retention failure must not report a completed save as failed.
    return {"written": [item["path"] for item in changed], "backups": backups}


class ModelListError(ValueError):
    """Only deliberately sanitized messages may cross the SSH boundary."""


class NoModelRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def model_headers(provider, wire):
    headers, secrets = {}, []

    def remote_env(name):
        value = os.environ.get(name)
        if not value or not value.strip():
            raise ModelListError(
                "远端 SSH 非交互会话的环境变量 {} 未设置或为空；"
                "请让该变量对 SSH 会话可见，助手不会额外加载 shell 配置或使用本机密钥。".format(name)
            )
        secrets.append(value)
        return value

    token = provider.get("bearer_token", "")
    if provider.get("env_key"):
        token = remote_env(provider["env_key"])
    elif token:
        secrets.append(token)
    if token:
        headers["x-api-key" if wire == "anthropic" else "authorization"] = (
            token if wire == "anthropic" else "Bearer " + token
        )
    for key, value in provider.get("headers", []):
        headers[key.lower()] = value
    for key, name in provider.get("env_http_headers", []):
        headers[key.lower()] = remote_env(name)
    if wire == "anthropic":
        headers.setdefault("anthropic-version", "2023-06-01")
    headers["content-type"] = "application/json"
    for key, value in headers.items():
        if not re.fullmatch(r"[!#$%&'*+\-.^_`|~0-9a-z]+", key):
            raise ModelListError("请求头名称不合法，未发送请求。")
        if any(ord(char) < 32 or ord(char) == 127 for char in value):
            raise ModelListError("请求头值包含控制字符，未发送请求。")
        try:
            value.encode("latin-1")
        except UnicodeEncodeError:
            raise ModelListError("请求头值不是有效的 HTTP 字符，未发送请求。") from None
    return headers, secrets


def probe_provider(request, chat=False):
    """Bounded GET/POST: authentication and generated content stay remote."""
    started = time.monotonic()

    def outcome(ok, status, summary, ids=None):
        return {
            "ok": ok, "status": status, "summary": summary,
            # Never echo response bodies, URLs, headers or environment values.
            "detail": "", "elapsed_ms": int((time.monotonic() - started) * 1000),
            "remote_models": ids or [],
        }

    provider = request.get("provider", {})
    wire = provider.get("wire_api", "responses")
    if wire not in ("responses", "chat", "anthropic"):
        raise ModelListError("不支持此 wire_api，未发送请求。")
    if any(provider.get(key) for key in ("requires_openai_auth", "command_auth", "aws_auth")):
        raise ModelListError(
            "远程服务商请求暂不支持 Codex 登录态、命令或 AWS 认证；"
            "不会读取登录凭据或执行认证命令。"
        )
    raw_base = provider.get("base_url", "")
    if any(ord(char) < 32 or ord(char) == 127 for char in raw_base):
        raise ModelListError("base_url 包含控制字符，未发送请求。")
    base = raw_base.strip().rstrip("/")
    try:
        parsed = urlsplit(base)
        valid = (
            parsed.scheme in ("http", "https") and parsed.hostname
            and parsed.username is None and parsed.password is None
            and (parsed.port is None or 0 < parsed.port <= 65535)
            and not any(char.isspace() for char in base)
        )
    except ValueError:
        valid = False
    if not valid:
        raise ModelListError("base_url 必须是有效的 HTTP(S) 地址，且不能包含用户名或密码。")
    if "?" in base or "#" in base:
        raise ModelListError("base_url 不能包含查询参数或片段；请使用 query_params。")
    payload = None
    if chat:
        model = request.get("model") or "test"
        if not isinstance(model, str) or len(model) > 4096:
            raise ModelListError("测试模型名无效")
        if wire == "responses":
            url = base + "/responses"
            payload = {"model": model, "input": "ping", "max_output_tokens": 16}
        else:
            url = base + (
                ("/messages" if base.endswith("/v1") else "/v1/messages")
                if wire == "anthropic" else "/chat/completions"
            )
            payload = {"model": model, "messages": [{"role": "user", "content": "ping"}], "max_tokens": 1}
        payload["stream"] = False
    else:
        url = base + ("/v1/models" if wire == "anthropic" and not base.endswith("/v1") else "/models")
    headers, secrets = model_headers(provider, wire)
    query = provider.get("query_params", [])
    if query:
        url += "?" + urlencode([tuple(pair) for pair in query])
    # build_opener uses the remote process's proxy environment. HTTPS retains
    # certificate/hostname checks; redirects never receive authentication data.
    opener = urllib.request.build_opener(
        NoModelRedirect(), urllib.request.HTTPSHandler(context=ssl.create_default_context())
    )
    try:
        req = urllib.request.Request(
            url, headers=headers, method="POST" if chat else "GET",
            data=json.dumps(payload).encode("utf-8") if chat else None,
        )
        response = opener.open(req, timeout=HTTP_TIMEOUT)
    except urllib.error.HTTPError as error:
        with error:
            status = error.code
        reason = {
            401: "未授权，请检查远端 API Key",
            403: "没有权限访问此接口",
            404: "接口不存在，请检查 base_url、协议或模型名",
            429: "请求过于频繁或额度用尽",
        }.get(status, "服务商请求失败")
        if 300 <= status < 400:
            reason = "重定向：为保护密钥未自动跟随，请检查 base_url"
        return outcome(False, status, "HTTP {} {}".format(status, reason))
    except Exception:
        return outcome(False, 0, "远端网络请求失败或超时，请检查地址、网络、代理和证书。")
    with response:
        status = response.status
        if not 200 <= status < 300:
            return outcome(False, status, "HTTP {} 服务商请求失败".format(status))
        try:
            length = response.headers.get("Content-Length")
            if length is not None and int(length) > MAX_HTTP_BODY:
                return outcome(False, status, "服务商响应超过 2 MiB，已拒绝读取。")
            body = response.read(MAX_HTTP_BODY + 1)
            if len(body) > MAX_HTTP_BODY:
                return outcome(False, status, "服务商响应超过 2 MiB，已拒绝读取。")
            if length is not None and len(body) != int(length):
                return outcome(False, status, "服务商响应不完整，请重试。")
        except Exception:
            return outcome(False, status, "读取远端响应失败或超时，响应可能不完整。")
    try:
        value = json.loads(body.decode("utf-8"))
    except (ValueError, UnicodeError):
        value = None
    if chat:
        valid = isinstance(value, dict) and "error" not in value and (
            (wire == "chat" and isinstance(value.get("choices"), list))
            or (wire == "responses" and isinstance(value.get("output"), list))
            or (wire == "anthropic" and value.get("type") == "message" and isinstance(value.get("content"), list))
        )
        return outcome(valid, status, "连接成功，服务商正常返回了响应。" if valid else
                       "HTTP 返回成功，但响应与所选协议不匹配。")
    if (
        not isinstance(value, dict) or "error" in value
        or not any(isinstance(value.get(key), list) for key in ("data", "models"))
    ):
        return outcome(False, status, "HTTP 返回成功，但响应不是有效的模型列表 JSON。")
    candidates = [
        entry.get("id") for entry in value.get("data", [])
        if isinstance(entry, dict) and isinstance(entry.get("id"), str)
    ] if isinstance(value.get("data"), list) else []
    if not candidates and isinstance(value.get("models"), list):
        for entry in value["models"]:
            if isinstance(entry, str):
                candidates.append(entry)
            elif isinstance(entry, dict):
                candidate = entry.get("id")
                candidates.append(candidate if isinstance(candidate, str) else entry.get("slug"))
    # A provider can echo authentication data even inside a model ID. Such
    # entries must not turn this operation into a credential extraction channel.
    ids = list(dict.fromkeys(
        candidate for candidate in candidates
        if isinstance(candidate, str) and candidate.strip()
        and not any(secret in candidate for secret in secrets)
    ))
    return outcome(True, status, "连接成功，发现 {} 个模型".format(len(ids)), ids)


def run(request):
    operation = request["operation"]
    if operation == "env_status":
        return environment_status(request)
    if operation in ("browse", "stat_path"):
        return inspect_path(request, browse=operation == "browse")
    if operation == "save":
        return save(request)
    if operation in ("list_models", "probe_chat"):
        try:
            return probe_provider(request, chat=operation == "probe_chat")
        except ModelListError:
            raise
        except Exception:
            # Library exception text may embed a URL or a credential value.
            raise ValueError("远端服务商请求失败，请检查服务商设置、网络和证书。") from None
    if operation != "load":
        raise ValueError("未知操作")
    user_home = os.path.expanduser("~")
    home = resolve(user_home, request.get("home") or os.environ.get("CODEX_HOME") or "~/.codex")
    config_path = os.path.join(home, "config.toml")
    config = read_file(config_path)
    if request.get("check_config"):
        unchanged(config_path, request.get("expected_config"))
    raw = request.get("catalog")
    catalog_path = resolve(home, raw) if raw else None
    if catalog_path == config_path:
        raise ValueError("模型目录不能与 config.toml 使用同一个文件")
    return {
        "home": home,
        "user_home": user_home,
        "config": config,
        "catalog_path": catalog_path,
        "catalog": read_file(catalog_path) if catalog_path else None,
    }


try:
    payload = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    if len(payload) > 32 * 1024 * 1024:
        raise ValueError("请求超过 32 MiB")
    result = run(json.loads(payload))
    print(json.dumps({"result": result}, ensure_ascii=False))
except Exception as error:
    print(json.dumps({"error": str(error)}, ensure_ascii=False))
    sys.exit(1)
