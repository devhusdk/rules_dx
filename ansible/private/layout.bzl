"""Path layout classifier mirroring ansible-lint 26.10.1 DEFAULT_KINDS."""

ANSIBLE_KINDS = [
    "playbook",
    "tasks",
    "handlers",
    "vars",
    "meta",
    "meta-runtime",
    "role-arg-spec",
    "play-argspec",
    "inventory",
    "requirements",
    "galaxy",
    "execution-environment",
    "rulebook",
    "test-meta",
]

DEFAULT_KINDS = [
    ("jinja2", "**/*.j2"),
    ("jinja2", "**/*.j2.*"),
    ("yaml", ".github/**/*.{yaml,yml}"),
    ("text", "**/templates/**/*.*"),
    ("execution-environment", "**/execution-environment.yml"),
    ("ansible-lint-config", "**/.ansible-lint"),
    ("ansible-lint-config", "**/.ansible-lint.{yaml,yml}"),
    ("ansible-lint-config", "**/.config/ansible-lint.{yaml,yml}"),
    ("ansible-navigator-config", "**/ansible-navigator.{yaml,yml}"),
    ("inventory", "**/inventory/**.{yaml,yml}"),
    ("requirements", "**/meta/requirements.{yaml,yml}"),
    ("galaxy", "**/galaxy.yml"),
    ("reno", "**/releasenotes/*/*.{yaml,yml}"),
    ("vars", "**/{host_vars,group_vars,vars,defaults}/**/*.{yaml,yml}"),
    ("tasks", "**/tasks/**/*.{yaml,yml}"),
    ("rulebook", "**/rulebooks/*.{yml,yaml}"),
    ("play-argspec", "**/*.meta.{yaml,yml}"),
    ("playbook", "**/playbooks/*.{yml,yaml}"),
    ("playbook", "**/*playbook*.{yml,yaml}"),
    ("playbook", "**/extensions/patterns/*/playbooks/*.{yml,yaml}"),
    ("handlers", "**/handlers/*.{yaml,yml}"),
    ("test-meta", "**/tests/integration/targets/*/meta/main.{yaml,yml}"),
    ("meta", "**/meta/main.{yaml,yml}"),
    ("meta-runtime", "**/meta/runtime.{yaml,yml}"),
    ("role-arg-spec", "**/meta/argument_specs.{yaml,yml}"),
    ("yaml", ".config/molecule/config.{yaml,yml}"),
    ("requirements", "**/molecule/*/{collections,requirements}.{yaml,yml}"),
    ("yaml", "**/molecule/*/{base,molecule}.{yaml,yml}"),
    ("requirements", "**/requirements.{yaml,yml}"),
    ("playbook", "**/molecule/*/*.{yaml,yml}"),
    ("yaml", "**/{.ansible-lint,.yamllint}"),
    ("changelog", "**/changelogs/changelog.{yaml,yml}"),
    ("yaml", "**/*.{yaml,yml}"),
    ("yaml", "**/.*.{yaml,yml}"),
]

def _expand_braces(pattern):
    pending = [pattern]
    done = []
    for _ in range(8):
        next_pending = []
        for item in pending:
            start = item.find("{")
            if start < 0:
                done.append(item)
                continue
            end = item.find("}", start)
            for alt in item[start + 1:end].split(","):
                next_pending.append(item[:start] + alt + item[end + 1:])
        pending = next_pending
        if len(pending) == 0:
            break
    return done

def _segment_match(pattern, text):
    parts = pattern.split("*")
    if len(parts) == 1:
        return pattern == text
    if not text.startswith(parts[0]):
        return False
    rest = text[len(parts[0]):]
    last = parts[-1]
    for mid in parts[1:-1]:
        idx = rest.find(mid)
        if idx < 0:
            return False
        rest = rest[idx + len(mid):]
    return len(rest) >= len(last) and rest.endswith(last)

def _glob_match(pattern, segments):
    pat = pattern.split("/")
    n = len(pat)
    m = len(segments)
    table = [[False] * (m + 1) for _ in range(n + 1)]
    table[n][m] = True
    for i in range(n - 1, -1, -1):
        for j in range(m, -1, -1):
            p = pat[i]
            if p == "**":
                table[i][j] = table[i + 1][j] or (j < m and table[i][j + 1])
            elif p.startswith("**."):
                table[i][j] = j < m and _segment_match("*" + p[2:], segments[j]) and (table[i + 1][j + 1] or table[i][j + 1])
                if not table[i][j] and j < m:
                    table[i][j] = table[i][j + 1]
            else:
                table[i][j] = j < m and _segment_match(p, segments[j]) and table[i + 1][j + 1]
    return table[0][0]

def ansible_layout_kind(short_path):
    """Returns the first matching ansible-lint kind for a path, or an empty string."""
    segments = [s for s in short_path.split("/") if s != ""]
    for kind, pattern in DEFAULT_KINDS:
        for expanded in _expand_braces(pattern):
            if _glob_match(expanded, segments):
                return kind
    return ""

def ansible_layout_class(short_path, forced):
    """Returns 'ansible' for Ansible layout paths or forced playbooks, else 'yaml'."""
    if forced or ansible_layout_kind(short_path) in ANSIBLE_KINDS:
        return "ansible"
    return "yaml"
