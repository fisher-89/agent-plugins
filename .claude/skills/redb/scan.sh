#!/usr/bin/env bash
# redb-peek 的批量驱动（macOS / Linux）：扫描一个数据根目录下的所有 .redb，
# 跨库汇总表与行数。与 scan.ps1 同一套输出形态。
#
# 用法:
#   ./scan.sh ~/.dev-team
#   ./scan.sh --details ~/.dev-team
#
# --details 会把每个库的表清单也打出来（默认只给每库一行摘要）。
# 二进制不存在时会先跑 ./build.sh。
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exe="$here/target/release/redb-peek"
# Windows 上用 Git Bash 跑时产物带 .exe（`-x` 测试不认 PATHEXT 补全）
if [ ! -x "$exe" ] && [ -x "$exe.exe" ]; then
  exe="$exe.exe"
fi

details=0
root=""
for arg in "$@"; do
  case "$arg" in
    -d|--details|-Details) details=1 ;;
    -h|--help)
      sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*)
      echo "scan.sh: 未知选项 $arg" >&2
      exit 2
      ;;
    *) root="$arg" ;;
  esac
done

if [ -z "$root" ]; then
  echo "用法: ./scan.sh [--details] <数据根目录>" >&2
  exit 2
fi

if [ ! -x "$exe" ]; then
  echo "redb-peek 未构建，先跑 ./build.sh" >&2
  "$here/build.sh" >&2
fi

if [ ! -d "$root" ]; then
  echo "数据根目录不存在: $root" >&2
  exit 2
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "scan.sh 需要 python3 解析 tables --jsonl 输出" >&2
  exit 2
fi

root="$(cd "$root" && pwd)"

# 收集 .redb 文件（与 scan.ps1 一致：递归、按路径排序、跳过不可读项）
dbs=()
while IFS= read -r f; do
  dbs+=("$f")
done < <(find "$root" -type f -name '*.redb' 2>/dev/null | LC_ALL=C sort)

if [ ${#dbs[@]} -eq 0 ]; then
  echo "在 $root 下没有找到 .redb 文件"
  exit 0
fi

echo "== 扫描 ${#dbs[@]} 个库 @ $root"
echo

ok=0
skipped=()
grand_tables=0
grand_rows=0

for db in "${dbs[@]}"; do
  rel="${db#"$root"}"
  rel="${rel#/}"

  set +e
  out="$("$exe" "$db" tables --jsonl 2>&1)"
  rc=$?
  set -e

  if [ $rc -ne 0 ]; then
    echo "[跳过] $rel"
    printf '%s\n' "$out" | sed 's/^/       /'
    skipped+=("$rel")
    continue
  fi

  ok=$((ok + 1))

  # 单库摘要（表数 / 行数 / 最大业务表 / 可选明细）交给 python 一次算完。
  # 用 -c 而不是 heredoc：heredoc 会占掉 stdin，管道数据就进不去了。
  summary="$(printf '%s\n' "$out" | DETAILS="$details" python3 -c '
import json, os, sys

details = os.environ.get("DETAILS") == "1"
rows = [json.loads(line) for line in sys.stdin if line.strip()]
total = sum(r["rows"] for r in rows)
biz = [r for r in rows if r["table"] != "metadata"]
biggest = max(biz, key=lambda r: r["rows"], default=None)
hint = (
    "最大表 %s (%d 行)" % (biggest["table"], biggest["rows"])
    if biggest and biggest["rows"] > 0
    else "无业务数据行"
)
print("%d\t%d\t%s" % (len(rows), total, hint))
if details:
    for r in rows:
        kind = "idx" if r["kind"] == "multimap" else "tbl"
        if r.get("model_id") is not None:
            model = "id=%s v%s" % (r["model_id"], r["model_version"])
        else:
            model = "-"
        print("\t%-34s %-4s %8d  %s" % (r["table"], kind, r["rows"], model))
')"

  table_count="$(printf '%s\n' "$summary" | awk -F'\t' 'NR==1{print $1}')"
  row_total="$(printf '%s\n' "$summary" | awk -F'\t' 'NR==1{print $2}')"
  hint="$(printf '%s\n' "$summary" | awk -F'\t' 'NR==1{print $3}')"
  grand_tables=$((grand_tables + table_count))
  grand_rows=$((grand_rows + row_total))

  echo "[OK]   $rel"
  echo "       $table_count 张表, $row_total 行; $hint"
  if [ "$details" -eq 1 ]; then
    printf '%s\n' "$summary" | tail -n +2 | sed 's/^\t/         /'
  fi
done

echo
echo "== 汇总"
echo "  可读库: $ok / ${#dbs[@]}"
if [ ${#skipped[@]} -gt 0 ]; then
  echo "  跳过:   ${#skipped[@]}（见上方原因）"
fi
echo "  表总数: $grand_tables"
echo "  行总数: $grand_rows"
