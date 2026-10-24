export interface BindingKey { scan: number; extended: boolean; vk: number }
export interface Binding { keys: BindingKey[]; label: string; suppress: boolean }
export type BindingTarget = 'hold' | 'toggle'
