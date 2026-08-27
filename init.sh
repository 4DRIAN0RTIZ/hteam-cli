#!/usr/bin/env bash
# init.sh — Verificación e inicialización del entorno
#
# Este script lo ejecuta el agente al COMENZAR una sesión y antes de
# declarar cualquier tarea como `done`. Si falla, la sesión no debe avanzar.
#
# Salida esperada: códigos de salida claros y bloques marcados con [OK]/[FAIL].

set -u
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

ok()    { printf "${GREEN}[OK]${NC}    %s\n" "$1"; }
warn()  { printf "${YELLOW}[WARN]${NC}  %s\n" "$1"; }
fail()  { printf "${RED}[FAIL]${NC}  %s\n" "$1"; }

EXIT_CODE=0

echo "── 1. Verificando entorno ─────────────────────────────"

if ! command -v cargo >/dev/null 2>&1; then
  fail "cargo no está instalado"
  exit 1
fi
ok "cargo -> $(cargo --version)"

if ! cargo fmt --version >/dev/null 2>&1; then
  fail "rustfmt no está instalado (rustup component add rustfmt)"
  EXIT_CODE=1
else
  ok "rustfmt -> $(cargo fmt --version)"
fi

if ! cargo clippy --version >/dev/null 2>&1; then
  fail "clippy no está instalado (rustup component add clippy)"
  EXIT_CODE=1
else
  ok "clippy -> $(cargo clippy --version)"
fi

echo ""
echo "── 2. Verificando archivos base del arnés ──────────────"

for f in AGENTS.md feature_list.json progress/current.md docs/architecture.md docs/conventions.md docs/verification.md CHECKPOINTS.md; do
  if [ ! -f "$f" ]; then
    fail "Falta archivo base: $f"
    EXIT_CODE=1
  else
    ok "Existe $f"
  fi
done

echo ""
echo "── 3. Validando feature_list.json ──────────────────────"

python3 - <<'PY'
import json, sys
try:
    data = json.load(open("feature_list.json"))
    valid = {"pending", "in_progress", "done", "blocked"}
    in_progress = [f for f in data["features"] if f["status"] == "in_progress"]
    if len(in_progress) > 1:
        print(f"[FAIL]  Hay {len(in_progress)} features en in_progress (máximo 1)")
        sys.exit(1)
    for f in data["features"]:
        if f["status"] not in valid:
            print(f"[FAIL]  Estado inválido en feature {f['id']}: {f['status']}")
            sys.exit(1)
    print(f"[OK]    feature_list.json válido ({len(data['features'])} features)")
except Exception as e:
    print(f"[FAIL]  feature_list.json inválido: {e}")
    sys.exit(1)
PY

if [ $? -ne 0 ]; then EXIT_CODE=1; fi

echo ""
echo "── 4. Formato (cargo fmt --check) ──────────────────────"

if cargo fmt --check 2>&1; then
  ok "Formato correcto"
else
  fail "Hay archivos sin formatear — corré 'cargo fmt'"
  EXIT_CODE=1
fi

echo ""
echo "── 5. Lints (cargo clippy) ──────────────────────────────"

if cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -30; then
  ok "Clippy sin warnings"
else
  fail "Clippy encontró warnings/errores"
  EXIT_CODE=1
fi

echo ""
echo "── 6. Ejecutando tests ─────────────────────────────────"

if cargo test --all-features 2>&1 | tail -40; then
  ok "Todos los tests pasan"
else
  fail "Hay tests rotos"
  EXIT_CODE=1
fi

echo ""
echo "── 7. Resumen ──────────────────────────────────────────"

if [ $EXIT_CODE -eq 0 ]; then
  ok "Entorno listo. Puedes empezar a trabajar."
else
  fail "Entorno NO está listo. Resuelve los errores antes de avanzar."
fi

exit $EXIT_CODE
