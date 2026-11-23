"""Genera le icone della tray (scudo su fondo trasparente) in src-tauri/icons/tray/.

Uso: python tools/genera_icone_tray.py
Produce, per ogni stato (verde, ambra, rosso, grigio): un PNG 32x32 (usato dall'app)
e un ICO con 16/24/32/48 px. Produce anche l'icona dell'app (scudo blu con spunta bianca,
1024 px) in src-tauri/icons/sorgente/app-1024.png, da cui `pnpm tauri icon` ricava tutte
le misure. Richiede Pillow.
"""

from pathlib import Path

from PIL import Image, ImageDraw

DEST = Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "tray"
SORGENTE_APP = DEST.parent / "sorgente" / "app-1024.png"
GRANDE = 512  # disegno ad alta risoluzione, poi ridotto con filtro Lanczos

STATI = {
    "verde": ("#2fa866", "#1b7a48", "check"),
    "ambra": ("#f0a81c", "#b87800", "punto_esclamativo"),
    "rosso": ("#e04545", "#a62626", "croce"),
    "grigio": ("#8b93a1", "#5e6674", "trattino"),
}

# Icona dell'app: variante blu/neutra dello stesso scudo, con la spunta
APP = ("#2f6fe4", "#1b4aa8", "check")
GRANDE_APP = 1024


def scudo(s: float, margine: float) -> list[tuple[float, float]]:
    """Contorno dello scudo in coordinate 0..s (spalle piatte, punta in basso)."""
    m = margine
    larg = s - 2 * m
    x0, x1, xc = m, s - m, s / 2
    alto, basso = m, s - m
    spalla = alto + larg * 0.12
    punti = [(xc, alto), (x1, spalla)]
    # lato destro che scende e si chiude a punta (curva approssimata)
    for i in range(1, 17):
        t = i / 16
        x = x1 - (x1 - xc) * (t**2.2)
        y = spalla + (basso - spalla) * (t**0.8)
        punti.append((x, y))
    for i in range(15, -1, -1):
        t = i / 16
        x = x0 + (xc - x0) * (t**2.2)
        y = spalla + (basso - spalla) * (t**0.8)
        punti.append((x, y))
    punti.append((x0, spalla))
    return punti


def disegna(riempimento: str, bordo: str, simbolo: str, s: int = GRANDE) -> Image.Image:
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    esterno = scudo(s, s * 0.04)
    d.polygon(esterno, fill=bordo)
    interno = scudo(s, s * 0.10)
    d.polygon(interno, fill=riempimento)

    bianco = (255, 255, 255, 255)
    sp = s * 0.075  # spessore del tratto
    cx, cy = s / 2, s * 0.46
    if simbolo == "check":
        d.line([(s * 0.33, s * 0.47), (s * 0.45, s * 0.60), (s * 0.68, s * 0.34)], fill=bianco, width=int(sp * 1.3), joint="curve")
    elif simbolo == "punto_esclamativo":
        d.rounded_rectangle([cx - sp * 0.9, s * 0.27, cx + sp * 0.9, s * 0.56], radius=sp * 0.8, fill=bianco)
        d.ellipse([cx - sp * 1.0, s * 0.62, cx + sp * 1.0, s * 0.62 + sp * 2.0], fill=bianco)
    elif simbolo == "croce":
        d.line([(s * 0.36, s * 0.30), (s * 0.64, s * 0.58)], fill=bianco, width=int(sp * 1.3))
        d.line([(s * 0.64, s * 0.30), (s * 0.36, s * 0.58)], fill=bianco, width=int(sp * 1.3))
    elif simbolo == "trattino":
        d.rounded_rectangle([s * 0.34, cy - sp * 0.7, s * 0.66, cy + sp * 0.7], radius=sp * 0.6, fill=bianco)
    return img


def main() -> None:
    DEST.mkdir(parents=True, exist_ok=True)
    for nome, (riemp, bordo, simbolo) in STATI.items():
        grande = disegna(riemp, bordo, simbolo)
        png = grande.resize((32, 32), Image.LANCZOS)
        png.save(DEST / f"tray-{nome}.png")
        grande.save(DEST / f"tray-{nome}.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48)])
        print("creato", DEST / f"tray-{nome}.png")

    SORGENTE_APP.parent.mkdir(parents=True, exist_ok=True)
    disegna(*APP, s=GRANDE_APP).save(SORGENTE_APP)
    print("creato", SORGENTE_APP)

    # Anteprima ingrandita (solo per controllo a occhio): 16 px e 32 px a 8x
    foglio = Image.new("RGBA", (4 * 280, 280 + 150), (30, 34, 42, 255))
    for i, nome in enumerate(STATI):
        g = disegna(*STATI[nome])
        for j, px in enumerate((32, 16)):
            piccolo = g.resize((px, px), Image.LANCZOS)
            ingr = piccolo.resize((px * 8, px * 8), Image.NEAREST)
            foglio.alpha_composite(ingr, (i * 280 + 10, 10 + j * 140 if j == 0 else 270))
    foglio.save(DEST / "_anteprima.png")


if __name__ == "__main__":
    main()
