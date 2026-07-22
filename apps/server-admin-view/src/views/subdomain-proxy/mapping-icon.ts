export const MAX_MAPPING_ICON_SOURCE_BYTES = 5 * 1024 * 1024;
export const MAX_MAPPING_ICON_OUTPUT_BYTES = 128 * 1024;

const ACCEPTED_MAPPING_ICON_MIME_TYPES = new Set([
  "image/png",
  "image/x-png",
  "image/jpeg",
  "image/jpg",
  "image/pjpeg",
  "image/webp",
  "image/x-webp",
  "image/avif",
  "image/svg+xml",
  "image/x-icon",
  "image/vnd.microsoft.icon",
]);
const ACCEPTED_MAPPING_ICON_EXTENSIONS = new Set([
  "png",
  "jpg",
  "jpeg",
  "webp",
  "avif",
  "svg",
  "ico",
]);

export const MAPPING_ICON_FILE_ACCEPT =
  ".png,.jpg,.jpeg,.webp,.avif,.svg,.ico,image/png,image/jpeg,image/webp,image/avif,image/svg+xml,image/x-icon,image/vnd.microsoft.icon";

export type MappingIconFileValidationIssue =
  | "unsupported_format"
  | "source_too_large"
  | null;

export class MappingIconProcessingError extends Error {
  readonly kind:
    | Exclude<MappingIconFileValidationIssue, null>
    | "decode_failed"
    | "encode_failed"
    | "output_too_large";

  constructor(kind: MappingIconProcessingError["kind"]) {
    super(kind);
    this.name = "MappingIconProcessingError";
    this.kind = kind;
  }
}

export const getMappingIconFileValidationIssue = (
  file: Pick<File, "name" | "size" | "type">,
): MappingIconFileValidationIssue => {
  if (file.size > MAX_MAPPING_ICON_SOURCE_BYTES) return "source_too_large";
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  const mediaType = file.type.toLowerCase();
  if (
    !ACCEPTED_MAPPING_ICON_MIME_TYPES.has(mediaType) &&
    !ACCEPTED_MAPPING_ICON_EXTENSIONS.has(extension)
  ) {
    return "unsupported_format";
  }
  return null;
};

const FORBIDDEN_SVG_ELEMENTS = new Set([
  "animate",
  "animatemotion",
  "animatetransform",
  "audio",
  "discard",
  "embed",
  "foreignobject",
  "iframe",
  "object",
  "script",
  "set",
  "video",
]);
const SAFE_EMBEDDED_SVG_IMAGE = /^data:image\/(?:png|jpe?g|webp|gif);base64,/i;
const ALLOWED_SVG_DOCTYPE =
  /<!doctype\s+svg(?:\s+(?:system\s+(?:"[^"]*"|'[^']*')|public\s+(?:"[^"]*"|'[^']*')\s+(?:"[^"]*"|'[^']*')))?\s*>/i;
const SVG_PROLOG_PREFIX =
  /^\uFEFF?(?:\s|<\?[\s\S]*?\?>|<!--[\s\S]*?-->)*$/;

const hasExternalSvgUrl = (value: string): boolean => {
  const urlPattern = /url\(\s*(?:"([^"]*)"|'([^']*)'|([^)]*))\s*\)/gi;
  for (const match of value.matchAll(urlPattern)) {
    const reference = (match[1] ?? match[2] ?? match[3] ?? "").trim();
    if (!reference.startsWith("#")) return true;
  }
  return false;
};

export const prepareMappingIconSvgSource = (sourceText: string): string => {
  if (/<!entity/i.test(sourceText)) {
    throw new MappingIconProcessingError("decode_failed");
  }

  const doctypeMatch = sourceText.match(ALLOWED_SVG_DOCTYPE);
  let preparedSource = sourceText;
  if (doctypeMatch?.index !== undefined) {
    const prefix = sourceText.slice(0, doctypeMatch.index);
    if (!SVG_PROLOG_PREFIX.test(prefix)) {
      throw new MappingIconProcessingError("decode_failed");
    }
    preparedSource =
      sourceText.slice(0, doctypeMatch.index) +
      sourceText.slice(doctypeMatch.index + doctypeMatch[0].length);
  }

  // A remaining declaration is malformed, duplicated, uses a non-SVG root, or
  // contains an internal subset. Do not pass any of those forms to DOMParser.
  if (/<!doctype/i.test(preparedSource)) {
    throw new MappingIconProcessingError("decode_failed");
  }
  return preparedSource;
};

const sanitizeSvgFile = async (file: File): Promise<Blob> => {
  const sourceText = prepareMappingIconSvgSource(await file.text());
  const documentNode = new DOMParser().parseFromString(
    sourceText,
    "image/svg+xml",
  );
  if (
    documentNode.querySelector("parsererror") ||
    documentNode.documentElement.localName.toLowerCase() !== "svg"
  ) {
    throw new MappingIconProcessingError("decode_failed");
  }

  const root = documentNode.documentElement;
  const viewBox = root
    .getAttribute("viewBox")
    ?.trim()
    .split(/[\s,]+/)
    .map(Number);
  const viewBoxWidth = viewBox?.[2] ?? 0;
  const viewBoxHeight = viewBox?.[3] ?? 0;
  if (
    viewBox?.length === 4 &&
    Number.isFinite(viewBoxWidth) &&
    Number.isFinite(viewBoxHeight) &&
    viewBoxWidth > 0 &&
    viewBoxHeight > 0 &&
    (!root.hasAttribute("width") ||
      !root.hasAttribute("height") ||
      root.getAttribute("width")?.includes("%") ||
      root.getAttribute("height")?.includes("%"))
  ) {
    root.setAttribute("width", String(viewBoxWidth));
    root.setAttribute("height", String(viewBoxHeight));
  }

  const elements = [root, ...Array.from(root.querySelectorAll("*"))];
  for (const element of elements) {
    if (FORBIDDEN_SVG_ELEMENTS.has(element.localName.toLowerCase())) {
      element.remove();
      continue;
    }
    if (
      element.localName.toLowerCase() === "style" &&
      (/@import/i.test(element.textContent ?? "") ||
        hasExternalSvgUrl(element.textContent ?? ""))
    ) {
      element.remove();
      continue;
    }
    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.trim();
      if (name.startsWith("on")) {
        element.removeAttribute(attribute.name);
        continue;
      }
      if (name === "href" || name === "xlink:href") {
        const isLocalReference = value.startsWith("#");
        const isSafeEmbeddedImage =
          element.localName.toLowerCase() === "image" &&
          SAFE_EMBEDDED_SVG_IMAGE.test(value);
        if (value && !isLocalReference && !isSafeEmbeddedImage) {
          element.removeAttribute(attribute.name);
        }
        continue;
      }
      if (hasExternalSvgUrl(value)) {
        element.removeAttribute(attribute.name);
      }
    }
  }

  return new Blob([new XMLSerializer().serializeToString(root)], {
    type: "image/svg+xml",
  });
};

const isSvgFile = (file: File) =>
  file.type.toLowerCase() === "image/svg+xml" ||
  file.name.toLowerCase().endsWith(".svg");

const loadImageElement = (sourceFile: Blob): Promise<HTMLImageElement> =>
  new Promise((resolve, reject) => {
    const source = URL.createObjectURL(sourceFile);
    const image = new Image();
    image.onload = () => {
      URL.revokeObjectURL(source);
      resolve(image);
    };
    image.onerror = () => {
      URL.revokeObjectURL(source);
      reject(new MappingIconProcessingError("decode_failed"));
    };
    image.src = source;
  });

type DecodedMappingIcon = {
  close?: () => void;
  height: number;
  source: CanvasImageSource;
  width: number;
};

const decodeMappingIcon = async (
  sourceFile: Blob,
  allowImageBitmap = true,
): Promise<DecodedMappingIcon> => {
  if (allowImageBitmap && typeof createImageBitmap === "function") {
    try {
      const bitmap = await createImageBitmap(sourceFile, {
        imageOrientation: "from-image",
      });
      if (bitmap.width && bitmap.height) {
        return {
          close: () => bitmap.close(),
          height: bitmap.height,
          source: bitmap,
          width: bitmap.width,
        };
      }
      bitmap.close();
    } catch {
      // Some browsers decode a format through <img> but not createImageBitmap.
    }
  }

  const image = await loadImageElement(sourceFile);
  if (!image.naturalWidth || !image.naturalHeight) {
    throw new MappingIconProcessingError("decode_failed");
  }
  return {
    height: image.naturalHeight,
    source: image,
    width: image.naturalWidth,
  };
};

const canvasToBlob = (
  canvas: HTMLCanvasElement,
  quality: number,
): Promise<Blob> =>
  new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (blob) resolve(blob);
        else reject(new MappingIconProcessingError("encode_failed"));
      },
      "image/webp",
      quality,
    );
  });

const blobToDataUrl = (blob: Blob): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () =>
      typeof reader.result === "string"
        ? resolve(reader.result)
        : reject(new MappingIconProcessingError("encode_failed"));
    reader.onerror = () =>
      reject(new MappingIconProcessingError("encode_failed"));
    reader.readAsDataURL(blob);
  });

export const processMappingIconFile = async (file: File): Promise<string> => {
  const validationIssue = getMappingIconFileValidationIssue(file);
  if (validationIssue) throw new MappingIconProcessingError(validationIssue);

  const sourceIsSvg = isSvgFile(file);
  const sourceFile = sourceIsSvg ? await sanitizeSvgFile(file) : file;
  const image = await decodeMappingIcon(sourceFile, !sourceIsSvg);

  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  if (!context) {
    image.close?.();
    throw new MappingIconProcessingError("encode_failed");
  }

  const attempts = [
    { size: 256, quality: 0.92 },
    { size: 256, quality: 0.82 },
    { size: 256, quality: 0.72 },
    { size: 192, quality: 0.78 },
    { size: 128, quality: 0.72 },
  ];
  try {
    for (const attempt of attempts) {
      canvas.width = attempt.size;
      canvas.height = attempt.size;
      context.clearRect(0, 0, attempt.size, attempt.size);
      const scale = Math.min(
        attempt.size / image.width,
        attempt.size / image.height,
      );
      const width = image.width * scale;
      const height = image.height * scale;
      context.drawImage(
        image.source,
        (attempt.size - width) / 2,
        (attempt.size - height) / 2,
        width,
        height,
      );
      const blob = await canvasToBlob(canvas, attempt.quality);
      if (blob.size <= MAX_MAPPING_ICON_OUTPUT_BYTES) {
        return await blobToDataUrl(blob);
      }
    }
  } finally {
    image.close?.();
  }

  throw new MappingIconProcessingError("output_too_large");
};
