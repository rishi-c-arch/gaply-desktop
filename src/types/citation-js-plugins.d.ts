declare module '@citation-js/core' {
  export class Cite {
    constructor(data?: unknown, options?: unknown);
    data: Record<string, unknown>[];
    format(type: string, options?: Record<string, unknown>): string;
    static async(input: string): Promise<Cite>;
  }
}

declare module '@citation-js/plugin-bibtex';
declare module '@citation-js/plugin-ris';
declare module '@citation-js/plugin-csl';
declare module '@citation-js/plugin-doi';
