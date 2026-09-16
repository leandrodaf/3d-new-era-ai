#!/usr/bin/env python3
"""Confere os números que as peças declaram no próprio nome.

Uma planta de marcenaria é lida pelos rótulos: "módulo 70 cm", "nicho 61 × 87",
"aéreo de 118 cm". Esses números vivem no NOME da peça, e nada os verifica —
`annotations(stale=true)` só olha labels amarrados com `about`, e uma planta
inteira pode não ter nenhum. Redimensionar a peça deixa o nome mentindo, em
silêncio, e quem vai cortar a chapa lê o nome.

Lê o estado pela API REST do editor (sem gastar contexto do agente) e aponta
cada nome cuja medida não bate com a geometria.

    python3 conferir-medidas-nos-nomes.py [--url http://127.0.0.1:7878/api/home]
                                          [--nivel lv3] [--tol 1.0]
"""

import argparse
import json
import re
import sys
import urllib.request

# "80,5 cm", "118 cm", "65,83 cm" — um número solto seguido de cm.
UMA_MEDIDA = re.compile(r"(\d+(?:[.,]\d+)?)\s*cm\b", re.I)
# "61 × 87", "45 L × 56 P × 85 A", "90 × 45 × 82,5" — medidas encadeadas.
ENCADEADA = re.compile(
    r"(\d+(?:[.,]\d+)?)\s*(?:[LPA])?\s*[x×]\s*(\d+(?:[.,]\d+)?)\s*(?:[LPA])?"
    r"(?:\s*[x×]\s*(\d+(?:[.,]\d+)?)\s*(?:[LPA])?)?",
    re.I,
)


def cm(texto):
    return float(texto.replace(",", "."))


def medidas_do_nome(nome):
    """Todo número em cm que o nome afirma, encadeados primeiro."""
    achados = []
    for m in ENCADEADA.finditer(nome):
        achados.extend(cm(g) for g in m.groups() if g)
    if achados:
        return achados
    return [cm(m.group(1)) for m in UMA_MEDIDA.finditer(nome)]


def bate(valor, peca, tol):
    """A medida aparece em alguma dimensão da peça (ou na soma de um vão)?"""
    for dimensao in (peca["width"], peca["depth"], peca["height"]):
        if abs(dimensao - valor) <= tol:
            return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", default="http://127.0.0.1:7878/api/home")
    ap.add_argument("--nivel", default=None, help="só este nível, ex. lv3")
    ap.add_argument("--tol", type=float, default=1.0, help="tolerância cm")
    args = ap.parse_args()

    try:
        with urllib.request.urlopen(args.url, timeout=10) as r:
            casa = json.load(r)["home"]
    except OSError as erro:
        sys.exit(f"não consegui ler {args.url}: {erro}\n(o editor está aberto?)")

    suspeitas = []
    for peca in casa["furniture"]:
        if args.nivel and peca.get("level") != args.nivel:
            continue
        nome = peca.get("name") or ""
        valores = medidas_do_nome(nome)
        if not valores:
            continue
        soltos = [v for v in valores if not bate(v, peca, args.tol)]
        if soltos:
            suspeitas.append((peca, soltos))

    if not suspeitas:
        print("nenhum nome contradiz a peça.")
        return

    print(f"{len(suspeitas)} peça(s) cujo nome afirma medida que não está nela:\n")
    for peca, soltos in suspeitas:
        d = (peca["width"], peca["depth"], peca["height"])
        print(f"  {peca['id']:>6}  {peca['name']}")
        print(f"         é {d[0]:g} × {d[1]:g} × {d[2]:g} cm; sem par para: "
              f"{', '.join(f'{v:g}' for v in soltos)}")


if __name__ == "__main__":
    main()
