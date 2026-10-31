import { mount } from 'svelte'
import App from './App.svelte'
import '@fontsource/anton/latin-400.css'
import '@fontsource/fragment-mono/latin-400.css'
import '@fontsource/vt323/latin-400.css'
import './app.css'

mount(App, { target: document.getElementById('app')! })
