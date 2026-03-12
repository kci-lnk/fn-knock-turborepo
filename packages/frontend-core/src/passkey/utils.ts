function base64UrlToUint8Array(value: string) {
  const padding = '='.repeat((4 - (value.length % 4)) % 4);
  const base64 = (value + padding).replace(/-/g, '+').replace(/_/g, '/');
  const raw = atob(base64);
  const array = new Uint8Array(raw.length);

  for (let i = 0; i < raw.length; i += 1) {
    array[i] = raw.charCodeAt(i);
  }

  return array;
}

function arrayBufferToBase64Url(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  bytes.forEach((value) => {
    binary += String.fromCharCode(value);
  });
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

export function normalizeCreationOptions(options: any) {
  return {
    ...options,
    challenge: base64UrlToUint8Array(options.challenge),
    user: {
      ...options.user,
      id: base64UrlToUint8Array(options.user.id),
    },
    excludeCredentials: options.excludeCredentials?.map((credential: any) => ({
      ...credential,
      id: base64UrlToUint8Array(credential.id),
    })),
  };
}

export function normalizeRequestOptions(options: any) {
  return {
    ...options,
    challenge: base64UrlToUint8Array(options.challenge),
    allowCredentials: options.allowCredentials?.map((credential: any) => ({
      ...credential,
      id: base64UrlToUint8Array(credential.id),
    })),
  };
}

export function serializeCredential(credential: PublicKeyCredential) {
  const response =
    credential.response as
      | AuthenticatorAttestationResponse
      | AuthenticatorAssertionResponse;
  const base = {
    id: credential.id,
    rawId: arrayBufferToBase64Url(credential.rawId),
    type: credential.type,
    clientExtensionResults: credential.getClientExtensionResults(),
  };

  if ('attestationObject' in response) {
    return {
      ...base,
      transports: (response as any).getTransports?.(),
      response: {
        attestationObject: arrayBufferToBase64Url(response.attestationObject),
        clientDataJSON: arrayBufferToBase64Url(response.clientDataJSON),
      },
    };
  }

  const assertion = response as AuthenticatorAssertionResponse;
  return {
    ...base,
    response: {
      authenticatorData: arrayBufferToBase64Url(assertion.authenticatorData),
      clientDataJSON: arrayBufferToBase64Url(assertion.clientDataJSON),
      signature: arrayBufferToBase64Url(assertion.signature),
      userHandle: assertion.userHandle
        ? arrayBufferToBase64Url(assertion.userHandle)
        : null,
    },
  };
}
