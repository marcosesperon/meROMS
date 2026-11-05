// Letras de la barra «saltar a letra»: «#» agrupa lo que empieza por número o símbolo
export const LETTERS = ["#", ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"];

/** Letra de un nombre: la primera, sin tilde y en mayúscula («Ñ» → «N»), o «#» si no es una letra */
export function letterOf(text: string): string {
  const first = text.trim().normalize("NFD").replace(/[̀-ͯ]/g, "").charAt(0).toUpperCase();
  return first >= "A" && first <= "Z" ? first : "#";
}
