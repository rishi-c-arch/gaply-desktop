import React from 'react';

const logos = [
  { name: 'Stanford', width: 120 },
  { name: 'MIT', width: 80 },
  { name: 'Harvard', width: 110 },
  { name: 'Oxford', width: 100 },
  { name: 'Cambridge', width: 130 },
  { name: 'Berkeley', width: 120 }
];

const testimonials = [
  {
    quote: "GAPLY has revolutionized how we conduct literature reviews. The AI-powered search saves us weeks of work.",
    author: "Dr. Sarah Chen",
    role: "Research Director",
    institution: "Stanford University",
    avatar: "https://images.unsplash.com/photo-1494790108755-2616b612b786?w=64&h=64&fit=crop&crop=face"
  },
  {
    quote: "The accuracy and speed of paper discovery is unmatched. It's become essential to our research workflow.",
    author: "Prof. Michael Rodriguez",
    role: "Department Head",
    institution: "MIT",
    avatar: "https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=64&h=64&fit=crop&crop=face"
  },
  {
    quote: "Finally, a tool that understands academic research. The journal matching feature is incredibly precise.",
    author: "Dr. Emily Watson",
    role: "Postdoc Researcher",
    institution: "Oxford University",
    avatar: "https://images.unsplash.com/photo-1438761681033-6461ffad8d80?w=64&h=64&fit=crop&crop=face"
  }
];

export default function TrustSection() {
  return (
    <section className="bg-neutral-50 py-20">
      <div className="max-w-6xl mx-auto px-6">
        {/* Trusted by section */}
        <div className="text-center mb-16">
          <p className="text-sm font-medium text-neutral-600 uppercase tracking-wider mb-8">
            Trusted by researchers worldwide
          </p>
          
          {/* Logos grid */}
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-8 items-center opacity-60">
            {logos.map((logo, index) => (
              <div
                key={index}
                className="flex items-center justify-center h-12 grayscale hover:grayscale-0 transition-all duration-300"
              >
                <div 
                  className="text-neutral-700 font-bold text-lg"
                  style={{ width: logo.width }}
                >
                  {logo.name}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Testimonials */}
        <div className="space-y-12">
          <div className="text-center">
            <h2 className="text-3xl md:text-4xl font-medium text-neutral-900 mb-4">
              What researchers are saying
            </h2>
            <p className="text-xl text-neutral-600 max-w-2xl mx-auto">
              Join thousands of academics who've accelerated their research with GAPLY
            </p>
          </div>

          <div className="grid md:grid-cols-3 gap-8">
            {testimonials.map((testimonial, index) => (
              <div
                key={index}
                className="bg-white rounded-lg p-8 shadow-soft border border-neutral-200 hover:shadow-pop transition-all duration-300"
              >
                <div className="space-y-6">
                  {/* Quote */}
                  <blockquote className="text-lg text-neutral-700 leading-relaxed">
                    "{testimonial.quote}"
                  </blockquote>

                  {/* Author */}
                  <div className="flex items-center space-x-4">
                    <img
                      src={testimonial.avatar}
                      alt={testimonial.author}
                      className="w-12 h-12 rounded-full object-cover"
                    />
                    <div>
                      <div className="font-semibold text-neutral-900">
                        {testimonial.author}
                      </div>
                      <div className="text-sm text-neutral-600">
                        {testimonial.role}
                      </div>
                      <div className="text-sm text-primary font-medium">
                        {testimonial.institution}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Stats */}
        <div className="mt-16 grid grid-cols-2 md:grid-cols-4 gap-8 text-center">
          <div className="space-y-2">
            <div className="text-3xl font-bold text-primary">50K+</div>
            <div className="text-sm text-neutral-600 font-medium">Active Researchers</div>
          </div>
          <div className="space-y-2">
            <div className="text-3xl font-bold text-primary">1M+</div>
            <div className="text-sm text-neutral-600 font-medium">Papers Analyzed</div>
          </div>
          <div className="space-y-2">
            <div className="text-3xl font-bold text-primary">98%</div>
            <div className="text-sm text-neutral-600 font-medium">User Satisfaction</div>
          </div>
          <div className="space-y-2">
            <div className="text-3xl font-bold text-primary">24/7</div>
            <div className="text-sm text-neutral-600 font-medium">Support Available</div>
          </div>
        </div>
      </div>
    </section>
  );
}
