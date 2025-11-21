import React from 'react';
import styled from 'styled-components';

interface KeyboardShortcutsProps {
  isOpen: boolean;
  onClose: () => void;
}

const Overlay = styled.div<{ isOpen: boolean }>`
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: ${props => props.isOpen ? 'flex' : 'none'};
  align-items: center;
  justify-content: center;
  z-index: 1000;
`;

const Dialog = styled.div`
  background: #2d2d2d;
  border-radius: 8px;
  padding: 24px;
  width: 500px;
  max-width: 90vw;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
`;

const Header = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid #3c3c3c;
`;

const Title = styled.h2`
  color: #ffffff;
  font-size: 20px;
  font-weight: 600;
  margin: 0;
`;

const CloseButton = styled.button`
  background: none;
  border: none;
  color: #cccccc;
  font-size: 24px;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: all 0.2s;

  &:hover {
    background: #3c3c3c;
    color: #ffffff;
  }
`;

const ShortcutList = styled.div`
  display: flex;
  flex-direction: column;
  gap: 16px;
`;

const ShortcutGroup = styled.div`
  display: flex;
  flex-direction: column;
  gap: 8px;
`;

const GroupTitle = styled.h3`
  color: #cccccc;
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const ShortcutItem = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
`;

const ShortcutDescription = styled.span`
  color: #cccccc;
  font-size: 13px;
`;

const ShortcutKeys = styled.div`
  display: flex;
  gap: 4px;
`;

const Key = styled.kbd`
  background: #1e1e1e;
  border: 1px solid #3c3c3c;
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 11px;
  font-family: 'Courier New', monospace;
  color: #ffffff;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
`;

export const KeyboardShortcuts: React.FC<KeyboardShortcutsProps> = ({
  isOpen,
  onClose
}) => {
  if (!isOpen) return null;

  const shortcuts = [
    {
      group: 'Query Execution',
      items: [
        { description: 'Execute Query', keys: ['Ctrl', 'Enter'] },
        { description: 'Execute Query (Mac)', keys: ['Cmd', 'Enter'] },
        { description: 'Explain Query', keys: ['Ctrl', 'Shift', 'Enter'] },
        { description: 'Explain Query (Mac)', keys: ['Cmd', 'Shift', 'Enter'] },
      ]
    },
    {
      group: 'Editor',
      items: [
        { description: 'Format Query', keys: ['Ctrl', 'Shift', 'F'] },
        { description: 'Format Query (Mac)', keys: ['Cmd', 'Shift', 'F'] },
        { description: 'Indent with Tab', keys: ['Tab'] },
      ]
    },
    {
      group: 'Navigation',
      items: [
        { description: 'New Query Tab', keys: ['Ctrl', 'T'] },
        { description: 'New Query Tab (Mac)', keys: ['Cmd', 'T'] },
        { description: 'Close Tab', keys: ['Ctrl', 'W'] },
        { description: 'Close Tab (Mac)', keys: ['Cmd', 'W'] },
        { description: 'Next Tab', keys: ['Ctrl', 'Tab'] },
        { description: 'Previous Tab', keys: ['Ctrl', 'Shift', 'Tab'] },
      ]
    },
    {
      group: 'General',
      items: [
        { description: 'Show Keyboard Shortcuts', keys: ['Ctrl', '/'] },
        { description: 'Show Keyboard Shortcuts (Mac)', keys: ['Cmd', '/'] },
      ]
    }
  ];

  return (
    <Overlay isOpen={isOpen} onClick={onClose}>
      <Dialog onClick={(e) => e.stopPropagation()}>
        <Header>
          <Title>Keyboard Shortcuts</Title>
          <CloseButton onClick={onClose}>×</CloseButton>
        </Header>

        <ShortcutList>
          {shortcuts.map((group, idx) => (
            <ShortcutGroup key={idx}>
              <GroupTitle>{group.group}</GroupTitle>
              {group.items.map((item, itemIdx) => (
                <ShortcutItem key={itemIdx}>
                  <ShortcutDescription>{item.description}</ShortcutDescription>
                  <ShortcutKeys>
                    {item.keys.map((key, keyIdx) => (
                      <Key key={keyIdx}>{key}</Key>
                    ))}
                  </ShortcutKeys>
                </ShortcutItem>
              ))}
            </ShortcutGroup>
          ))}
        </ShortcutList>
      </Dialog>
    </Overlay>
  );
};

