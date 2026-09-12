"""Executed over SSH; JSON on stdin/stdout, Python 3 standard library only.

No configuration values are interpolated into shell commands.
"""
import datetime
import json
import os
import re
import stat
import sys
import tempfile
import time

MAX_FILE = 8 * 1024 * 1024


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


def run(request):
    if request["operation"] == "save":
        return save(request)
    if request["operation"] != "load":
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
