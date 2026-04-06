import { useToast } from 'primevue/usetoast'

export function useAppToast() {
  const toast = useToast()

  return {
    success: (message: string, title = 'Success') => {
      toast.add({
        severity: 'success',
        summary: title,
        detail: message,
        life: 3000
      })
    },
    error: (message: string, title = 'Error') => {
      toast.add({
        severity: 'error',
        summary: title,
        detail: message,
        life: 5000
      })
    },
    info: (message: string, title = 'Info') => {
      toast.add({
        severity: 'info',
        summary: title,
        detail: message,
        life: 3000
      })
    },
    warning: (message: string, title = 'Warning') => {
      toast.add({
        severity: 'warn',
        summary: title,
        detail: message,
        life: 4000
      })
    }
  }
}
