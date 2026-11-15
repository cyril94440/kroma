interface Box<T> {
  get(): T;
  set(next: T): void;
}

export function box<T>(initial: T): Box<T> {
  let value = initial;
  return {
    get: () => value,
    set: (next) => {
      value = next;
    },
  };
}
