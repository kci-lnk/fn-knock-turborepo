import {
  nextTick,
  onBeforeUnmount,
  ref,
  toValue,
  watch,
  type ComponentPublicInstance,
  type MaybeRefOrGetter,
} from "vue";
import {
  MATRIX_VERTEX_SOURCE,
  PROBE_BLUR_SOURCE,
  PROBE_COMPOSITE_SOURCE,
  PROBE_FIELD_SOURCE,
} from "./scanIntensityShaders";

type RenderTarget = { texture: WebGLTexture; framebuffer: WebGLFramebuffer };

type UseScanIntensityMatrixOptions = {
  active: MaybeRefOrGetter<boolean>;
  tier: MaybeRefOrGetter<number>;
};

export function useScanIntensityMatrix({
  active,
  tier,
}: UseScanIntensityMatrixOptions) {
  const canvasRef = ref<HTMLCanvasElement | null>(null);
  const isFallback = ref(false);

  function setCanvas(element: Element | ComponentPublicInstance | null) {
    canvasRef.value = element instanceof HTMLCanvasElement ? element : null;
  }

  let graphics: WebGL2RenderingContext | null = null;
  let geometryArray: WebGLVertexArrayObject | null = null;
  let geometryBuffer: WebGLBuffer | null = null;
  let fieldProgram: WebGLProgram | null = null;
  let blurProgram: WebGLProgram | null = null;
  let compositeProgram: WebGLProgram | null = null;
  let feedbackFront: RenderTarget | null = null;
  let feedbackBack: RenderTarget | null = null;
  let blurHorizontal: RenderTarget | null = null;
  let blurVertical: RenderTarget | null = null;
  let sizeObserver: ResizeObserver | null = null;
  let animationToken = 0;
  let loopActive = false;
  let visualTier = toValue(tier);
  let renderStartedAt = 0;
  let reducedMotion = false;

  function initializePortMatrix() {
    shutdownPortMatrix();
    const canvas = canvasRef.value;
    if (!canvas || !toValue(active)) return;
    const context = canvas.getContext("webgl2", {
      antialias: false,
      alpha: true,
      preserveDrawingBuffer: false,
      powerPreference: "low-power",
    });
    if (!context) {
      isFallback.value = true;
      return;
    }
    graphics = context;
    isFallback.value = false;
    reducedMotion = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    canvas.addEventListener("webglcontextlost", handleMatrixContextLost);
    canvas.addEventListener(
      "webglcontextrestored",
      handleMatrixContextRestored,
    );
    try {
      createMatrixPrograms();
      resizePortMatrix();
    } catch (error) {
      console.warn("scan intensity WebGL initialization failed", error);
      isFallback.value = true;
      shutdownPortMatrix();
      return;
    }
    sizeObserver = new ResizeObserver(resizePortMatrix);
    sizeObserver.observe(canvas);
    visualTier = toValue(tier);
    if (visualTier === 3) startPortWave();
  }

  function compileMatrixShader(kind: number, source: string) {
    if (!graphics) throw new Error("WebGL context unavailable");
    const shader = graphics.createShader(kind);
    if (!shader) throw new Error("Unable to allocate shader");
    graphics.shaderSource(shader, source);
    graphics.compileShader(shader);
    if (!graphics.getShaderParameter(shader, graphics.COMPILE_STATUS)) {
      const message =
        graphics.getShaderInfoLog(shader) || "Shader compilation failed";
      graphics.deleteShader(shader);
      throw new Error(message);
    }
    return shader;
  }

  function linkMatrixProgram(fragmentSource: string) {
    if (!graphics) throw new Error("WebGL context unavailable");
    const vertex = compileMatrixShader(
      graphics.VERTEX_SHADER,
      MATRIX_VERTEX_SOURCE,
    );
    const fragment = compileMatrixShader(
      graphics.FRAGMENT_SHADER,
      fragmentSource,
    );
    const program = graphics.createProgram();
    if (!program) throw new Error("Unable to allocate program");
    graphics.attachShader(program, vertex);
    graphics.attachShader(program, fragment);
    graphics.bindAttribLocation(program, 0, "a_vertex");
    graphics.linkProgram(program);
    graphics.deleteShader(vertex);
    graphics.deleteShader(fragment);
    if (!graphics.getProgramParameter(program, graphics.LINK_STATUS)) {
      const message =
        graphics.getProgramInfoLog(program) || "Program link failed";
      graphics.deleteProgram(program);
      throw new Error(message);
    }
    return program;
  }

  function createMatrixPrograms() {
    if (!graphics) return;
    fieldProgram = linkMatrixProgram(PROBE_FIELD_SOURCE);
    blurProgram = linkMatrixProgram(PROBE_BLUR_SOURCE);
    compositeProgram = linkMatrixProgram(PROBE_COMPOSITE_SOURCE);
    geometryArray = graphics.createVertexArray();
    geometryBuffer = graphics.createBuffer();
    graphics.bindVertexArray(geometryArray);
    graphics.bindBuffer(graphics.ARRAY_BUFFER, geometryBuffer);
    graphics.bufferData(
      graphics.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
      graphics.STATIC_DRAW,
    );
    graphics.enableVertexAttribArray(0);
    graphics.vertexAttribPointer(0, 2, graphics.FLOAT, false, 0, 0);
  }

  function createRenderTarget(width: number, height: number): RenderTarget {
    if (!graphics) throw new Error("WebGL context unavailable");
    const texture = graphics.createTexture();
    const framebuffer = graphics.createFramebuffer();
    if (!texture || !framebuffer)
      throw new Error("Unable to allocate render target");
    graphics.bindTexture(graphics.TEXTURE_2D, texture);
    graphics.texParameteri(
      graphics.TEXTURE_2D,
      graphics.TEXTURE_MIN_FILTER,
      graphics.LINEAR,
    );
    graphics.texParameteri(
      graphics.TEXTURE_2D,
      graphics.TEXTURE_MAG_FILTER,
      graphics.LINEAR,
    );
    graphics.texParameteri(
      graphics.TEXTURE_2D,
      graphics.TEXTURE_WRAP_S,
      graphics.CLAMP_TO_EDGE,
    );
    graphics.texParameteri(
      graphics.TEXTURE_2D,
      graphics.TEXTURE_WRAP_T,
      graphics.CLAMP_TO_EDGE,
    );
    graphics.texImage2D(
      graphics.TEXTURE_2D,
      0,
      graphics.RGBA,
      width,
      height,
      0,
      graphics.RGBA,
      graphics.UNSIGNED_BYTE,
      null,
    );
    graphics.bindFramebuffer(graphics.FRAMEBUFFER, framebuffer);
    graphics.framebufferTexture2D(
      graphics.FRAMEBUFFER,
      graphics.COLOR_ATTACHMENT0,
      graphics.TEXTURE_2D,
      texture,
      0,
    );
    if (
      graphics.checkFramebufferStatus(graphics.FRAMEBUFFER) !==
      graphics.FRAMEBUFFER_COMPLETE
    ) {
      graphics.deleteFramebuffer(framebuffer);
      graphics.deleteTexture(texture);
      throw new Error("Incomplete scan visualization framebuffer");
    }
    return { texture, framebuffer };
  }

  function resizePortMatrix() {
    const canvas = canvasRef.value;
    if (!graphics || !canvas) return;
    const bounds = canvas.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const scale = Math.min(window.devicePixelRatio || 1, 1.5);
    const width = Math.max(1, Math.round(bounds.width * scale));
    const height = Math.max(1, Math.round(bounds.height * scale));
    if (canvas.width === width && canvas.height === height && feedbackFront)
      return;
    canvas.width = width;
    canvas.height = height;
    releaseRenderTargets();
    feedbackFront = createRenderTarget(width, height);
    feedbackBack = createRenderTarget(width, height);
    blurHorizontal = createRenderTarget(width, height);
    blurVertical = createRenderTarget(width, height);
    for (const target of [
      feedbackFront,
      feedbackBack,
      blurHorizontal,
      blurVertical,
    ]) {
      graphics.bindFramebuffer(graphics.FRAMEBUFFER, target.framebuffer);
      graphics.clearColor(0, 0, 0, 1);
      graphics.clear(graphics.COLOR_BUFFER_BIT);
    }
    graphics.bindFramebuffer(graphics.FRAMEBUFFER, null);
    if (visualTier === 3) startPortWave();
  }

  function clearPortWaveTargets() {
    if (!graphics) return;
    for (const target of [
      feedbackFront,
      feedbackBack,
      blurHorizontal,
      blurVertical,
    ]) {
      if (!target) continue;
      graphics.bindFramebuffer(graphics.FRAMEBUFFER, target.framebuffer);
      graphics.clearColor(0, 0, 0, 1);
      graphics.clear(graphics.COLOR_BUFFER_BIT);
    }
    graphics.bindFramebuffer(graphics.FRAMEBUFFER, null);
    graphics.clearColor(0, 0, 0, 0);
    graphics.clear(graphics.COLOR_BUFFER_BIT);
  }

  function startPortWave() {
    if (!graphics || visualTier !== 3) return;
    if (animationToken) cancelAnimationFrame(animationToken);
    animationToken = 0;
    loopActive = false;
    renderStartedAt = performance.now();
    clearPortWaveTargets();
    requestMatrixFrames();
  }

  function stopPortWave() {
    if (animationToken) cancelAnimationFrame(animationToken);
    animationToken = 0;
    loopActive = false;
  }

  function bindTexture(unit: number, texture: WebGLTexture | null) {
    if (!graphics || !texture) return;
    graphics.activeTexture(graphics.TEXTURE0 + unit);
    graphics.bindTexture(graphics.TEXTURE_2D, texture);
  }

  function renderPortMatrix(timestamp: number) {
    if (
      !graphics ||
      !canvasRef.value ||
      !fieldProgram ||
      !blurProgram ||
      !compositeProgram ||
      !feedbackFront ||
      !feedbackBack ||
      !blurHorizontal ||
      !blurVertical
    ) {
      loopActive = false;
      return;
    }
    const width = canvasRef.value.width;
    const height = canvasRef.value.height;
    graphics.viewport(0, 0, width, height);
    graphics.bindVertexArray(geometryArray);

    graphics.bindFramebuffer(graphics.FRAMEBUFFER, feedbackBack.framebuffer);
    graphics.useProgram(fieldProgram);
    bindTexture(0, feedbackFront.texture);
    graphics.uniform1i(
      graphics.getUniformLocation(fieldProgram, "u_echoTexture"),
      0,
    );
    graphics.uniform1f(
      graphics.getUniformLocation(fieldProgram, "u_clockSeconds"),
      timestamp / 1000,
    );
    graphics.uniform1f(
      graphics.getUniformLocation(fieldProgram, "u_waveAge"),
      (timestamp - renderStartedAt) / 1000,
    );
    graphics.uniform1f(
      graphics.getUniformLocation(fieldProgram, "u_motionFactor"),
      reducedMotion ? 0 : 1,
    );
    graphics.drawArrays(graphics.TRIANGLES, 0, 6);

    graphics.bindFramebuffer(graphics.FRAMEBUFFER, blurHorizontal.framebuffer);
    graphics.useProgram(blurProgram);
    bindTexture(0, feedbackBack.texture);
    graphics.uniform1i(
      graphics.getUniformLocation(blurProgram, "u_sourceFrame"),
      0,
    );
    graphics.uniform2f(
      graphics.getUniformLocation(blurProgram, "u_blurAxis"),
      1,
      0,
    );
    graphics.uniform2f(
      graphics.getUniformLocation(blurProgram, "u_frameSize"),
      width,
      height,
    );
    graphics.drawArrays(graphics.TRIANGLES, 0, 6);

    graphics.bindFramebuffer(graphics.FRAMEBUFFER, blurVertical.framebuffer);
    bindTexture(0, blurHorizontal.texture);
    graphics.uniform2f(
      graphics.getUniformLocation(blurProgram, "u_blurAxis"),
      0,
      1,
    );
    graphics.drawArrays(graphics.TRIANGLES, 0, 6);

    graphics.bindFramebuffer(graphics.FRAMEBUFFER, null);
    graphics.useProgram(compositeProgram);
    bindTexture(0, feedbackBack.texture);
    bindTexture(1, blurVertical.texture);
    graphics.uniform1i(
      graphics.getUniformLocation(compositeProgram, "u_probeFrame"),
      0,
    );
    graphics.uniform1i(
      graphics.getUniformLocation(compositeProgram, "u_haloFrame"),
      1,
    );
    graphics.drawArrays(graphics.TRIANGLES, 0, 6);

    [feedbackFront, feedbackBack] = [feedbackBack, feedbackFront];
    const keepAnimating = !reducedMotion && visualTier === 3;
    if (keepAnimating && toValue(active)) {
      animationToken = requestAnimationFrame(renderPortMatrix);
    } else {
      animationToken = 0;
      loopActive = false;
    }
  }

  function requestMatrixFrames() {
    if (!graphics || loopActive || !toValue(active) || visualTier !== 3) return;
    loopActive = true;
    animationToken = requestAnimationFrame(renderPortMatrix);
  }

  function releaseRenderTargets() {
    if (graphics) {
      for (const target of [
        feedbackFront,
        feedbackBack,
        blurHorizontal,
        blurVertical,
      ]) {
        if (!target) continue;
        graphics.deleteFramebuffer(target.framebuffer);
        graphics.deleteTexture(target.texture);
      }
    }
    feedbackFront = null;
    feedbackBack = null;
    blurHorizontal = null;
    blurVertical = null;
  }

  function releaseMatrixPrograms() {
    if (graphics) {
      for (const program of [fieldProgram, blurProgram, compositeProgram]) {
        if (program) graphics.deleteProgram(program);
      }
      if (geometryBuffer) graphics.deleteBuffer(geometryBuffer);
      if (geometryArray) graphics.deleteVertexArray(geometryArray);
    }
    fieldProgram = null;
    blurProgram = null;
    compositeProgram = null;
    geometryBuffer = null;
    geometryArray = null;
  }

  function shutdownPortMatrix() {
    if (animationToken) cancelAnimationFrame(animationToken);
    animationToken = 0;
    loopActive = false;
    sizeObserver?.disconnect();
    sizeObserver = null;
    const canvas = canvasRef.value;
    canvas?.removeEventListener("webglcontextlost", handleMatrixContextLost);
    canvas?.removeEventListener(
      "webglcontextrestored",
      handleMatrixContextRestored,
    );
    releaseRenderTargets();
    releaseMatrixPrograms();
    graphics = null;
  }

  function handleMatrixContextLost(event: Event) {
    event.preventDefault();
    if (animationToken) cancelAnimationFrame(animationToken);
    animationToken = 0;
    loopActive = false;
  }

  function handleMatrixContextRestored() {
    shutdownPortMatrix();
    void nextTick(initializePortMatrix);
  }

  watch(
    () => toValue(active),
    async (isActive) => {
      if (!isActive) {
        shutdownPortMatrix();
        return;
      }
      await nextTick();
      initializePortMatrix();
    },
    { immediate: true },
  );

  watch(
    () => toValue(tier),
    (value) => {
      visualTier = value;
      if (value === 3) {
        startPortWave();
      } else {
        stopPortWave();
      }
    },
    { immediate: true },
  );

  onBeforeUnmount(shutdownPortMatrix);

  return {
    setCanvas,
    isFallback,
    initialize: initializePortMatrix,
    shutdown: shutdownPortMatrix,
  };
}
