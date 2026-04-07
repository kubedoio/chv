<script lang="ts">
  import { goto } from '$app/navigation';
  import { createAPIClient } from '$lib/api/client';

  let token = '';
  let tokenName = 'admin';
  let error = '';
  let message = '';
  const client = createAPIClient();

  async function saveExistingToken() {
    try {
      client.setToken(token);
      await client.validateLogin();
      await goto('/');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Login failed.';
    }
  }

  async function createAndSaveToken() {
    try {
      const result = await client.createToken(tokenName);
      token = result.token;
      message = result.message;
      await saveExistingToken();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Token creation failed.';
    }
  }
</script>

<div class="flex min-h-screen items-center justify-center bg-chrome p-6">
  <div class="w-full max-w-lg table-card">
    <div class="card-header px-6 py-4">
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">CHV Login</div>
      <div class="mt-1 text-xl font-semibold">Bearer Token Access</div>
    </div>

    <div class="space-y-5 p-6">
      <label class="block">
        <span class="mb-2 block text-sm text-muted">Token name</span>
        <input bind:value={tokenName} class="w-full border border-line px-3 py-2 text-sm" />
      </label>

      <label class="block">
        <span class="mb-2 block text-sm text-muted">Existing token</span>
        <textarea bind:value={token} rows="5" class="mono w-full border border-line px-3 py-2 text-sm"></textarea>
      </label>

      {#if error}
        <div class="border border-danger bg-red-50 px-3 py-3 text-sm text-danger">{error}</div>
      {/if}

      {#if message}
        <div class="border border-success bg-green-50 px-3 py-3 text-sm text-success">{message}</div>
      {/if}

      <div class="flex flex-wrap gap-3">
        <button class="button-primary px-4 py-2 text-sm font-medium" on:click|preventDefault={saveExistingToken}>Use Token</button>
        <button class="button-secondary px-4 py-2 text-sm font-medium" on:click|preventDefault={createAndSaveToken}>Create Token</button>
      </div>
    </div>
  </div>
</div>

