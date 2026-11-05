// Artículo del nombre al final o al principio: «The Legend of Zelda» ↔ «Legend of Zelda, The».
// Así los juegos se ordenan por la palabra importante. Solo se mira la primera palabra o la
// coletilla «, X» del final; las etiquetas de detrás («(USA)», «[!]») se quedan al final.

const ARTICLES = ["The", "A", "An", "El", "La", "Los", "Las"];

/** Separa «Nombre (USA) [!]» en «Nombre» y « (USA) [!]» */
function splitTags(name: string): [string, string] {
  const match = name.match(/^(.*?)(\s*[([].*)$/);
  return match && match[1].trim() ? [match[1].trimEnd(), match[2]] : [name, ""];
}

/** «The Legend of Zelda (USA)» → «Legend of Zelda, The (USA)»; si no empieza por artículo, null */
export function articleToEnd(name: string): string | null {
  const [title, tags] = splitTags(name.trim());
  const space = title.indexOf(" ");
  if (space <= 0) return null;
  const first = title.slice(0, space);
  const article = ARTICLES.find((a) => a.toLowerCase() === first.toLowerCase());
  const rest = title.slice(space + 1).trim();
  if (!article || !rest) return null;
  return `${rest}, ${first}${tags}`;
}

/** «Legend of Zelda, The (USA)» → «The Legend of Zelda (USA)»; si no acaba en «, artículo», null */
export function articleToFront(name: string): string | null {
  const [title, tags] = splitTags(name.trim());
  const comma = title.lastIndexOf(", ");
  if (comma <= 0) return null;
  const last = title.slice(comma + 2).trim();
  if (!ARTICLES.some((a) => a.toLowerCase() === last.toLowerCase())) return null;
  return `${last} ${title.slice(0, comma).trim()}${tags}`;
}
