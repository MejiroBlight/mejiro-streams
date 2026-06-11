import { Menu, MenuItem, Submenu } from '@tauri-apps/api/menu';
import { Window } from '@tauri-apps/api/window';

export async function createMenubar(window: Window) {
    const fileSubmenu = await Submenu.new({
        text: 'File',
        items: [
            await MenuItem.new({
                id: 'new',
                text: 'New',
                action: () => {
                    console.log('New clicked');
                },
            }),
            await MenuItem.new({
                id: 'open',
                text: 'Open',
                action: () => {
                    console.log('Open clicked');
                },
            }),
            await MenuItem.new({
                id: 'save_as',
                text: 'Save As...',
                action: () => {
                    console.log('Save As clicked');
                },
            }),
        ],
    });

    const editSubmenu = await Submenu.new({
        text: 'Edit',
        items: [
            await MenuItem.new({
                id: 'undo',
                text: 'Undo',
                action: () => {
                    console.log('Undo clicked');
                },
            }),
            await MenuItem.new({
                id: 'redo',
                text: 'Redo',
                action: () => {
                    console.log('Redo clicked');
                },
            }),
        ],
    });

    const menu = await Menu.new({
        items: [
            fileSubmenu,
            editSubmenu,
            await MenuItem.new({
                id: 'quit',
                text: 'Quit',
                action: () => {
                    console.log('Quit pressed');
                },
            }),
        ],
    });

    menu.setAsWindowMenu(window);
}