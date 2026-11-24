import type { ScrapeMiss } from "../commands";
import { translate } from "../i18n";

/**
 * «No encontrado por el scraper el …»: el texto de la marca, con la fecha en el formato del
 * idioma de la app. Lo usan la insignia (como aviso al pasar el ratón), la tabla y la ficha.
 */
export function scrapeMissText(miss: ScrapeMiss, locale: string): string {
  const date = new Date(miss.at * 1000).toLocaleDateString(locale);
  return translate(miss.manual ? "scrapeMiss.markedOn" : "scrapeMiss.notFoundOn", { date });
}
