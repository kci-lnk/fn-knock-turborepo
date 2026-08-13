export const MATRIX_VERTEX_SOURCE = `#version 300 es
layout(location = 0) in vec2 a_vertex;
out vec2 v_coordinate;
void main() {
  v_coordinate = a_vertex * 0.5 + 0.5;
  gl_Position = vec4(a_vertex, 0.0, 1.0);
}`;

export const PROBE_FIELD_SOURCE = `#version 300 es
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

export const PROBE_BLUR_SOURCE = `#version 300 es
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

export const PROBE_COMPOSITE_SOURCE = `#version 300 es
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
