import { type StyleProp, StyleSheet, type ViewStyle } from 'react-native';

const HIDDEN = { tabBarItemStyle: { display: 'none' } } as const;

/** Options that keep a tab's route but take it out of the tab bar, for a tab
 *  that exists only when the server runs the module behind it. */
export function tabOptions(visible: boolean): { tabBarItemStyle?: StyleProp<ViewStyle> } {
  return visible ? {} : HIDDEN;
}

export function isHiddenTab(options: { tabBarItemStyle?: StyleProp<ViewStyle> }): boolean {
  return StyleSheet.flatten(options.tabBarItemStyle)?.display === 'none';
}
