declare module "nprogress" {
  export interface NProgressOptions {
    minimum?: number;
    easing?: string;
    positionUsing?: string;
    speed?: number;
    trickle?: boolean;
    trickleSpeed?: number;
    showSpinner?: boolean;
    barSelector?: string;
    spinnerSelector?: string;
    parent?: string;
    template?: string;
  }

  export interface NProgressStatic {
    configure(options: NProgressOptions): NProgressStatic;
    start(): NProgressStatic;
    done(force?: boolean): NProgressStatic;
    inc(amount?: number): NProgressStatic;
    set(value: number): NProgressStatic;
    remove(): void;
    isStarted(): boolean;
    status: number | null;
  }

  const NProgress: NProgressStatic;
  export default NProgress;
}
