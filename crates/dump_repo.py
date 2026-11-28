import os
from pathlib import Path

# Directories and files to ignore
IGNORE_DIRS = {
    ".git", ".svn", ".hg", "node_modules", "__pycache__", ".venv", "venv",
    "env", "dist", "build", "target", ".next", ".nuxt", ".idea", ".vscode",
    "coverage", ".pytest_cache", ".mypy_cache", "vendor", "bin", "obj"
}

IGNORE_FILES = {
    "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "Cargo.lock",
    "poetry.lock", "Gemfile.lock", "composer.lock", ".DS_Store", "repo_dump.txt"
}

# Only include files with these extensions (add or remove as needed)
ALLOWED_EXTS = {
    ".py", ".rs", ".js", ".ts", ".jsx", ".tsx", ".c", ".cpp", ".h", ".hpp",
    ".cs", ".go", ".java", ".kt", ".swift", ".rb", ".php", ".sh", ".ps1",
    ".bat", ".sql", ".html", ".css", ".scss", ".json", ".yaml", ".yml",
    ".toml", ".xml", ".md", ".txt", ".env.example", ".dockerfile"
}

# Filenames without standard extensions to still include
ALLOWED_FILENAMES = {
    "Dockerfile", "Makefile", "CMakeLists.txt", "Cargo.toml", "pyproject.toml"
}

def generate_tree(root_path: Path) -> str:
    lines = [f"{root_path.name}/"]
    for dirpath, dirnames, filenames in os.walk(root_path):
        dirnames[:] = sorted([d for d in dirnames if d not in IGNORE_DIRS])
        rel_dir = Path(dirpath).relative_to(root_path)
        depth = 0 if str(rel_dir) == "." else len(rel_dir.parts)
        indent = "    " * depth

        if str(rel_dir) != ".":
            lines.append(f"{indent}{rel_dir.name}/")

        sub_indent = "    " * (depth + 1)
        for f in sorted(filenames):
            if f not in IGNORE_FILES and (Path(f).suffix.lower() in ALLOWED_EXTS or f in ALLOWED_FILENAMES):
                lines.append(f"{sub_indent}{f}")
    return "\n".join(lines)

def dump_repository(root_dir: str = ".", output_file: str = "repo_dump.txt"):
    root_path = Path(root_dir).resolve()
    out_path = root_path / output_file
    file_count = 0

    with open(out_path, "w", encoding="utf-8") as out:
        # 1. Write Project Tree
        out.write("=" * 80 + "\n")
        out.write(f"REPOSITORY STRUCTURE: {root_path.name}\n")
        out.write("=" * 80 + "\n\n")
        out.write(generate_tree(root_path) + "\n\n")

        # 2. Walk and Dump File Contents
        for dirpath, dirnames, filenames in os.walk(root_path):
            # Prune ignored directories in-place
            dirnames[:] = sorted([d for d in dirnames if d not in IGNORE_DIRS])

            for filename in sorted(filenames):
                if filename in IGNORE_FILES:
                    continue

                file_path = Path(dirpath) / filename
                if file_path == out_path:
                    continue

                if file_path.suffix.lower() not in ALLOWED_EXTS and filename not in ALLOWED_FILENAMES:
                    continue

                rel_path = file_path.relative_to(root_path)
                try:
                    content = file_path.read_text(encoding="utf-8")
                    out.write("=" * 80 + "\n")
                    out.write(f"FILE: {rel_path.as_posix()}\n")
                    out.write("=" * 80 + "\n")
                    out.write(content)
                    if not content.endswith("\n"):
                        out.write("\n")
                    out.write("\n")
                    file_count += 1
                except UnicodeDecodeError:
                    # Skip any binary or non-UTF8 files that slipped through
                    continue

    size_kb = out_path.stat().st_size / 1024
    print(f"Done! Packed {file_count} files into '{out_path}' ({size_kb:.1f} KB).")

if __name__ == "__main__":
    dump_repository()
