import React, { useEffect, useState } from 'react';

export default function CustomCursor() {
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [isHovering, setIsHovering] = useState(false);
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    // Only enable on devices with fine pointer (desktop)
    if (!window.matchMedia('(pointer: fine)').matches) {
      return;
    }

    const handleMouseMove = (e: MouseEvent) => {
      setPosition({ x: e.clientX, y: e.clientY });
      setIsVisible(true);
    };

    const handleMouseEnter = () => {
      setIsVisible(true);
    };

    const handleMouseLeave = () => {
      setIsVisible(false);
    };

    const handleMouseOver = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.tagName === 'A' || 
          target.tagName === 'BUTTON' || 
          target.classList.contains('clickable') ||
          target.closest('button') ||
          target.closest('a')) {
        setIsHovering(true);
      } else {
        setIsHovering(false);
      }
    };

    // Add event listeners
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseenter', handleMouseEnter);
    document.addEventListener('mouseleave', handleMouseLeave);
    document.addEventListener('mouseover', handleMouseOver);

    // Hide default cursor
    document.body.style.cursor = 'none';

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseenter', handleMouseEnter);
      document.removeEventListener('mouseleave', handleMouseLeave);
      document.removeEventListener('mouseover', handleMouseOver);
      document.body.style.cursor = 'auto';
    };
  }, []);

  // Don't render on touch devices
  if (!window.matchMedia('(pointer: fine)').matches) {
    return null;
  }

  return (
    <div
      className={`fixed pointer-events-none z-[9999] transition-all duration-150 ease-out ${
        isVisible ? 'opacity-100' : 'opacity-0'
      }`}
      style={{
        left: position.x,
        top: position.y,
        transform: 'translate(-50%, -50%)',
      }}
    >
      {/* Main cursor dot */}
      <div
        className={`absolute w-3 h-3 bg-primary rounded-full transition-all duration-200 ${
          isHovering ? 'scale-150 bg-primary-600' : 'scale-100'
        }`}
        style={{
          boxShadow: isHovering 
            ? '0 0 20px rgba(0, 163, 255, 0.6)' 
            : '0 0 12px rgba(0, 163, 255, 0.4)'
        }}
      />
      
      {/* Outer glow ring */}
      <div
        className={`absolute w-6 h-6 border border-primary/30 rounded-full transition-all duration-300 ${
          isHovering ? 'scale-200 opacity-50' : 'scale-100 opacity-30'
        }`}
      />
      
      {/* Inner pulse ring */}
      <div
        className={`absolute w-8 h-8 border border-primary/20 rounded-full transition-all duration-500 ${
          isHovering ? 'scale-300 opacity-30' : 'scale-150 opacity-20'
        }`}
      />
    </div>
  );
}
