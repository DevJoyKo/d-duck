// Catalog types (app-language, D-171). The Korean files are the reference (`as const`); each English file must
// satisfy `Shape<typeof ko>` for the same namespace, so a missing or extra key fails `tsc`.

/** A count-dependent message. `other` is required; `one` (and the rest) are picked by Intl.PluralRules. The count
 * parameter is always `{count}`. Korean needs only `other`. */
export type Plural = { zero?: string; one?: string; two?: string; few?: string; many?: string; other: string };

export type Message = string | Plural;

/** A namespace tree: messages, or nested groups of them. */
export interface Tree {
  readonly [key: string]: Message | Tree;
}

/** What the other language must provide: the same keys, any wording. */
export type Shape<T> = { -readonly [K in keyof T]: T[K] extends string ? string : T[K] extends { other: string } ? Plural : Shape<T[K]> };

/** Dotted paths to every message under `T` (`"board.title"`). */
export type Paths<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string | { other: string } ? `${P}${K}` : Paths<T[K], `${P}${K}.`>;
}[keyof T & string];

/** The message at a dotted path. */
export type At<T, K extends string> = K extends `${infer H}.${infer R}` ? (H extends keyof T ? At<T[H], R> : never) : K extends keyof T ? T[K] : never;

/** `{name}` placeholders of a literal string. */
export type Placeholders<S> = S extends `${string}{${infer P}}${infer R}` ? P | Placeholders<R> : never;

type MessageParams<M> = M extends string ? Placeholders<M> : M extends { other: infer O } ? Placeholders<O> | "count" : never;

/** The rest arguments of `t()`: none for a plain message, a params object naming every placeholder otherwise.
 * Numbers are shown with the current locale's digit grouping; pass a string to keep a value as written. */
export type ArgsFor<M> = [MessageParams<M>] extends [never] ? [] : [params: Record<MessageParams<M>, string | number>];
