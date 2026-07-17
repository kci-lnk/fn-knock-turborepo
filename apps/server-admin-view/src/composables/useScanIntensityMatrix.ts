import {
  nextTick,
  onBeforeUnmount,
  ref,
  toValue,
  watch,
  type ComponentPublicInstance,
  type MaybeRefOrGetter,
} from "vue";

const MATRIX_VERTEX_SOURCE = `#version 300 es
layout(location = 0) in vec2 a_vertex;
out vec2 v_coordinate;
void main() {
  v_coordinate = a_vertex * 0.5 + 0.5;
  gl_Position = vec4(a_vertex, 0.0, 1.0);
}`;

const PROBE_FIELD_SOURCE = `#version 300 es
precision highp float;
in vec2 v_coordinate;
out vec4 outputColor;
uniform sampler2D u_echoTexture;
uniform float u_clockSeconds;
uniform float u_waveAge;
uniform float u_motionFactor;
float portNoise(vec2 coordinate) {
  return fract(sin(dot(coordinate, vec2(127.1, 311.7))) * 43758.5453);
}
void main() {
  vec2 matrixPosition = v_coordinate * vec2(72.0, 6.0);
  vec2 portCell = floor(matrixPosition);
  vec2 cellPosition = fract(matrixPosition);
  vec2 cellDistance = abs(cellPosition - 0.5);
  float cellShape = smoothstep(0.34, 0.22, max(cellDistance.x * 0.9, cellDistance.y));
  float seed = portNoise(portCell);

  float leftFade = smoothstep(0.0, 0.45, v_coordinate.x);
  vec3 echo = texture(u_echoTexture, v_coordinate).rgb * 0.90 * leftFade;
  float stableClock = mix(1.7, u_clockSeconds, u_motionFactor);
  float stableAge = mix(3.0, u_waveAge, u_motionFactor);
  float ignitionDelay = seed * 1.2;
  float lifetime = max(stableAge - ignitionDelay, 0.0);
  float ignited = step(0.001, lifetime);
  float probeSpeed = 0.85 + seed * 0.30;
  float acceleration = 1.0 - pow(1.0 - clamp(lifetime / 2.5, 0.0, 1.0), 3.0);
  float travelled = acceleration * probeSpeed * ignited;
  float leadingOffset = (seed - 0.5) * 0.05;
  float waveFront = max(1.0 - travelled - leadingOffset, 0.02);
  float wakeLength = max(1.0 - waveFront, 0.001);
  float insideWake = step(waveFront - 0.003, v_coordinate.x) * step(v_coordinate.x, 1.003);
  float wakeDepth = clamp(max(1.0 - v_coordinate.x, 0.0) / wakeLength, 0.0, 1.0);
  float leadingHeat = pow(1.0 - wakeDepth, 0.65);
  leadingHeat = max(leadingHeat, 0.04 * ignited) * insideWake;
  leadingHeat *= 1.0 - smoothstep(0.94, 1.05, wakeDepth);

  float ramp = mix(0.15, 0.5, min(stableAge, 1.0));
  float verticalDistance = abs(v_coordinate.y - 0.5) * 2.0;
  float verticalProfile = pow(max(1.0 - verticalDistance * verticalDistance * 0.45, 0.0), 0.75);
  float tempo = mix(0.85, 1.0, min(stableAge / 1.5, 1.0));
  float bandA = sin(v_coordinate.x * 30.0 + stableClock * 15.0 * tempo + seed * 6.28);
  float bandB = sin(v_coordinate.x * 17.0 + stableClock * 8.0 * tempo + seed * 3.14);
  float bandC = sin(v_coordinate.x * 52.0 + stableClock * 25.0 * tempo + seed * 10.0);
  float flicker = smoothstep(0.08, 0.92, (bandA + bandB * 0.5 + bandC * 0.25) * 0.35 + 0.5);
  float rhythmA = sin(wakeDepth * 16.0 - stableClock * 5.0 * tempo + seed * 3.0);
  float rhythmB = sin(wakeDepth * 8.0 - stableClock * 2.5 * tempo + seed * 5.0);
  float rhythm = smoothstep(-0.15, 0.55, rhythmA) * (rhythmB * 0.5 + 0.5);
  rhythm = pow(max(rhythm, 0.0), 1.2);

  float sparkProgress = fract(stableClock * (0.38 + seed * 0.15) + seed * 7.0);
  float sparkX = 1.0 - sparkProgress * wakeLength;
  float sparkY = 0.5 + sin(sparkProgress * 11.0 + seed * 6.28) * 0.28;
  float spark = smoothstep(0.014, 0.0, abs(v_coordinate.x - sparkX))
              * smoothstep(0.18, 0.0, abs(v_coordinate.y - sparkY))
              * pow(1.0 - sparkProgress, 2.0) * ramp;
  float energy = leadingHeat * verticalProfile * (flicker * 0.42 + rhythm * 0.38)
               + spark * 0.7 * insideWake;
  energy *= ramp;

  float frontGlow = exp(-pow((v_coordinate.x - waveFront) * 18.0, 2.0));
  float edgeFlicker = sin(v_coordinate.x * 45.0 + stableClock * 20.0 * tempo + seed * 6.28) * 0.5 + 0.5;
  float waveEdge = frontGlow * (0.25 + edgeFlicker * 1.5) * 1.6 * ramp;
  float distanceAhead = waveFront - v_coordinate.x;
  float aheadZone = smoothstep(0.07, 0.0, distanceAhead) * step(0.0, distanceAhead) * verticalProfile;
  float secondarySeed = portNoise(portCell + vec2(99.0, 33.0));
  float aheadSpark = aheadZone * step(0.6, secondarySeed)
                   * (sin(distanceAhead * 100.0 + stableClock * 20.0 * tempo + secondarySeed * 6.28) * 0.5 + 0.5)
                   * ramp * 0.5;

  float totalEnergy = energy + waveEdge + aheadSpark;
  vec3 deepViolet = vec3(0.28, 0.10, 0.58);
  vec3 brightViolet = vec3(0.62, 0.32, 1.0);
  vec3 whiteCore = vec3(1.0, 0.94, 0.98);
  float temperature = 1.0 - wakeDepth;
  vec3 color = mix(deepViolet, brightViolet, temperature);
  color = mix(color, whiteCore, pow(temperature, 4.5));
  color *= totalEnergy;
  float endpoint = exp(-pow((v_coordinate.x - 1.0) * 16.0, 2.0));
  color += whiteCore * endpoint * 2.2 * (sin(stableClock * 2.8) * 0.15 + 1.0) * ramp;
  color += brightViolet * exp(-pow((v_coordinate.x - 1.0) * 3.5, 2.0)) * 0.12 * ramp;
  color *= cellShape * leftFade;
  outputColor = vec4(min(echo + color, vec3(1.5)), 1.0);
}`;

const PROBE_BLUR_SOURCE = `#version 300 es
precision highp float;
in vec2 v_coordinate;
out vec4 outputColor;
uniform sampler2D u_sourceFrame;
uniform vec2 u_blurAxis;
uniform vec2 u_frameSize;
void main() {
  vec2 offset = u_blurAxis * 1.7 / u_frameSize;
  vec3 color = texture(u_sourceFrame, v_coordinate).rgb * 0.227027;
  color += texture(u_sourceFrame, v_coordinate + offset).rgb * 0.1945946;
  color += texture(u_sourceFrame, v_coordinate - offset).rgb * 0.1945946;
  color += texture(u_sourceFrame, v_coordinate + offset * 2.0).rgb * 0.1216216;
  color += texture(u_sourceFrame, v_coordinate - offset * 2.0).rgb * 0.1216216;
  outputColor = vec4(color, 1.0);
}`;

const PROBE_COMPOSITE_SOURCE = `#version 300 es
precision highp float;
in vec2 v_coordinate;
out vec4 outputColor;
uniform sampler2D u_probeFrame;
uniform sampler2D u_haloFrame;
void main() {
  vec3 probe = texture(u_probeFrame, v_coordinate).rgb;
  vec3 halo = texture(u_haloFrame, v_coordinate).rgb;
  vec3 mapped = 1.0 - exp(-(probe + halo * 1.15 + probe * halo * 0.25));
  outputColor = vec4(mapped, 1.0);
}`;

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
