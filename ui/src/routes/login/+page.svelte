<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { createAPIClient, getStoredToken } from '$lib/api/client';
  import { toast } from '$lib/stores/toast';

  let username = '';
  let password = '';
  let error = '';
  let loading = false;
  const client = createAPIClient();

  // Redirect if already logged in
  onMount(() => {
    if (getStoredToken()) {
      goto('/');
    }
  });

  async function handleLogin() {
    if (!username || !password) {
      error = 'Username and password are required';
      return;
    }

    loading = true;
    error = '';

    try {
      const response = await fetch('/api/v1/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password })
      });

      if (!response.ok) {
        const data = await response.json();
        throw new Error(data.error?.message || 'Login failed');
      }

      const data = await response.json();
      
      // Store token
      client.setToken(data.token);
      
      toast.success(`Welcome, ${data.user.username}!`);
      await goto('/');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Login failed';
      toast.error(error);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      handleLogin();
    }
  }
</script>

<div class="flex min-h-screen items-center justify-center bg-chrome p-6">
  <div class="w-full max-w-md table-card">
    <div class="card-header px-6 py-4">
      <div class="text-[11px] uppercase tracking-[0.16em] text-muted">CHV</div>
      <div class="mt-1 text-xl font-semibold">Sign In</div>
    </div>

    <div class="space-y-5 p-6">
      <div class="text-sm text-muted">
        Default credentials: admin / admin
      </div>

      <label class="block">
        <span class="mb-2 block text-sm text-muted">Username</span>
        <input 
          bind:value={username} 
          on:keydown={handleKeydown}
          class="w-full border border-line px-3 py-2 text-sm"
          placeholder="Enter username"
          autocomplete="username"
        />
      </label>

      <label class="block">
        <span class="mb-2 block text-sm text-muted">Password</span>
        <input 
          bind:value={password} 
          on:keydown={handleKeydown}
          type="password"
          class="w-full border border-line px-3 py-2 text-sm"
          placeholder="Enter password"
          autocomplete="current-password"
        />
      </label>

      {#if error}
        <div class="border border-danger bg-red-50 px-3 py-3 text-sm text-danger">{error}</div>
      {/if}

      <button 
        class="button-primary w-full px-4 py-2 text-sm font-medium"
        on:click|preventDefault={handleLogin}
        disabled={loading}
      >
        {#if loading}
          Signing in...
        {:else}
          Sign In
        {/if}
      </button>

      <div class="border-t border-line pt-4">
        <div class="text-xs text-muted">
          <p class="mb-1"><strong>First time?</strong> Use the default credentials above.</p>
          <p>You can also <a href="/" class="text-primary hover:underline">create an API token</a> directly.</p>
        </div>
      </div>
    </div>
  </div>
</div>
