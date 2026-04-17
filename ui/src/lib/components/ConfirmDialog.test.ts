import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { describe, expect, it, vi, afterEach } from 'vitest';
import ConfirmDialog from './ConfirmDialog.svelte';

describe('ConfirmDialog.svelte', () => {
	afterEach(() => {
		cleanup();
	});
    it('does not render when open is false', () => {
        render(ConfirmDialog, { 
            props: { 
                open: false, 
                title: 'Confirm', 
                description: 'Description', 
                onConfirm: vi.fn() 
            } 
        });
        expect(screen.queryByText('Confirm')).toBeNull();
    });

    it('renders title and description when open', () => {
        render(ConfirmDialog, { 
            props: { 
                open: true, 
                title: 'Confirm Action', 
                description: 'Are you sure you want to do this?', 
                onConfirm: vi.fn() 
            } 
        });
        expect(screen.getByText('Confirm Action')).toBeTruthy();
        expect(screen.getByText('Are you sure you want to do this?')).toBeTruthy();
    });

    it('emits onConfirm when confirm button is clicked', async () => {
        const handleConfirm = vi.fn();
        render(ConfirmDialog, { 
            props: { 
                open: true, 
                title: 'Confirm Title', 
                description: 'Desc', 
                confirmText: 'Yes, Confirm',
                onConfirm: handleConfirm 
            } 
        });
        
        await fireEvent.click(screen.getByText("Yes, Confirm"));
        expect(handleConfirm).toHaveBeenCalled();
    });
});
